// ssh/proxmox.rs — Remote Proxmox guest management primitives (LXC + QEMU VM)
//
// Pure logic layer: no SSH, no Tauri. All functions are synchronous
// and fully unit-testable without a live connection.
//
// Responsibilities:
//   1. parse_pct_list          — parse `pct list` whitespace-delimited table
//   2. parse_pct_listsnapshot  — parse `pct listsnapshot <vmid>` output
//   3. parse_qm_list           — parse `qm list` whitespace-delimited table (different columns)
//   4. validate_vmid           — injection-safe validator: digit-only char-loop + u32 range
//   5. validate_snapshot_name  — injection-safe validator: charset + length + starts-with-letter
//   6. build_lifecycle_command — compose pct/qm start/stop/reboot from GuestKind + validated vmid
//   7. build_listsnapshot_command, build_snapshot_command,
//      build_rollback_command, build_delsnapshot_command — all GuestKind-aware
//   8. is_pct_unavailable      — heuristic: not installed / permission denied
//
// INJECTION SAFETY (critical):
//   VMIDs are u32 integers (100–999999999). Pure digit-only char-loop + parse::<u32>()
//   + range check before storing. Command builders take the validated u32 — no raw
//   string interpolation from user input.
//
//   Snapshot names: pure char-loop — len 1..=40, first byte ASCII alphabetic,
//   rest ASCII alphanumeric | '_' | '-' (no dots). NO regex crate.
//
//   Parse-source defense-in-depth: parse_pct_list and parse_qm_list drop rows
//   whose VMID fails validate_vmid; parse_pct_listsnapshot drops snapshots whose
//   name fails validate_snapshot_name. No unsafe value ever reaches the store or
//   the shell command.

use serde::{Deserialize, Serialize};

use crate::error::AppError;

// ─── Types ───────────────────────────────────────────────────────────────────

/// A validated Proxmox container ID (CTID).
/// Stored as u32; range 100..=999_999_999.
pub type ProxmoxVmid = u32;

/// Discriminator for Proxmox guest kind.
///
/// Controls which CLI tool (`pct` or `qm`) the command builders emit.
/// `Lxc` is the default and mirrors all pre-existing behaviour.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GuestKind {
    /// LXC container — managed via `pct`.
    Lxc,
    /// QEMU virtual machine — managed via `qm`.
    Vm,
}

/// A single row from `pct list`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LxcRow {
    /// Container VMID (validated u32, 100..=999_999_999).
    pub vmid: ProxmoxVmid,
    /// Container status: "running", "stopped", etc.
    pub status: String,
    /// Container name (hostname).
    pub name: String,
}

/// A single row from `qm list`.
///
/// `qm list` columns: `VMID  NAME  STATUS  MEM(MB)  BOOTDISK(GB)  PID`
/// PID may be blank for stopped VMs (Proxmox emits an empty field).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VmRow {
    /// VM VMID (validated u32, 100..=999_999_999).
    pub vmid: ProxmoxVmid,
    /// VM name.
    pub name: String,
    /// VM status: "running", "stopped", etc.
    pub status: String,
}

/// A single snapshot from `pct listsnapshot <vmid>` / `qm listsnapshot <vmid>`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRow {
    /// Snapshot name (validated).
    pub name: String,
}

/// Lifecycle action a user can trigger on an LXC container or QEMU VM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum LxcAction {
    Start,
    Stop,
    Reboot,
}

// ─── VMID validation ─────────────────────────────────────────────────────────

/// Validate a Proxmox container VMID string.
///
/// Accepts ONLY strings where every byte is an ASCII digit, parses as u32,
/// and checks the valid Proxmox CTID range: 100..=999_999_999.
///
/// Returns the validated u32 on success, or an `AppError::Other` on failure.
///
/// # Security
/// Pure char-loop (no regex). All characters inspected individually. The u32
/// result is stored and used in all command builders — the raw string is
/// never passed to the shell after validation.
pub fn validate_vmid(s: &str) -> Result<ProxmoxVmid, AppError> {
    if s.is_empty() {
        return Err(AppError::Other("VMID is empty".to_string()));
    }
    // Every byte must be an ASCII digit.
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return Err(AppError::Other(format!(
                "Invalid VMID (injection guard): {s:?}"
            )));
        }
    }
    // Parse as u32 (overflow → invalid).
    let n: u32 = s
        .parse()
        .map_err(|_| AppError::Other(format!("Invalid VMID (overflow or parse error): {s:?}")))?;
    // Proxmox CTID range.
    if !(100..=999_999_999).contains(&n) {
        return Err(AppError::Other(format!(
            "VMID out of range (100–999999999): {n}"
        )));
    }
    Ok(n)
}

// ─── Snapshot name validation ─────────────────────────────────────────────────

/// Validate a Proxmox snapshot name before use in a shell command.
///
/// Rules:
///   - Length: 1..=40 bytes
///   - First byte: ASCII alphabetic (A-Z or a-z)
///   - Remaining bytes: ASCII alphanumeric, underscore (`_`), or hyphen (`-`)
///   - No dots, spaces, slashes, semicolons, or any other character
///
/// Returns `Ok(&str)` on success, `Err(AppError::Other)` on failure.
///
/// # Security
/// Pure char-loop — no regex dependency. Called at parse source and at
/// command boundary (defense-in-depth).
pub fn validate_snapshot_name(name: &str) -> Result<&str, AppError> {
    let bytes = name.as_bytes();

    match bytes.len() {
        0 => return Err(AppError::Other("Snapshot name is empty".to_string())),
        1..=40 => {}
        _ => {
            return Err(AppError::Other(format!(
                "Snapshot name too long (max 40): {name:?}"
            )))
        }
    }

    // First byte must be ASCII alphabetic.
    if !bytes[0].is_ascii_alphabetic() {
        return Err(AppError::Other(format!(
            "Invalid snapshot name (must start with a letter): {name:?}"
        )));
    }

    // Remaining bytes: alphanumeric | '_' | '-'
    for &b in &bytes[1..] {
        if !b.is_ascii_alphanumeric() && b != b'_' && b != b'-' {
            return Err(AppError::Other(format!(
                "Invalid snapshot name (injection guard): {name:?}"
            )));
        }
    }

    Ok(name)
}

// ─── Command builders ─────────────────────────────────────────────────────────

/// CLI tool name for a given guest kind.
///
/// `Lxc` → `"pct"`, `Vm` → `"qm"`.
/// All command builders use this to pick the right binary — the raw string
/// never comes from user input.
fn cli(kind: &GuestKind) -> &'static str {
    match kind {
        GuestKind::Lxc => "pct",
        GuestKind::Vm => "qm",
    }
}

/// Build `pct|qm start|stop|reboot <vmid>` from a validated VMID.
///
/// Passing `GuestKind::Lxc` produces the same output as before, preserving
/// backward compatibility for all existing LXC callers.
pub fn build_lifecycle_command(
    action: &LxcAction,
    vmid: ProxmoxVmid,
    kind: &GuestKind,
) -> String {
    let verb = match action {
        LxcAction::Start => "start",
        LxcAction::Stop => "stop",
        LxcAction::Reboot => "reboot",
    };
    format!("{} {verb} {vmid}", cli(kind))
}

/// Build `pct|qm listsnapshot <vmid>`.
pub fn build_listsnapshot_command(vmid: ProxmoxVmid, kind: &GuestKind) -> String {
    format!("{} listsnapshot {vmid}", cli(kind))
}

/// Build `pct|qm snapshot <vmid> <name>` (create snapshot).
pub fn build_snapshot_command(vmid: ProxmoxVmid, name: &str, kind: &GuestKind) -> String {
    format!("{} snapshot {vmid} {name}", cli(kind))
}

/// Build `pct|qm rollback <vmid> <name>`.
pub fn build_rollback_command(vmid: ProxmoxVmid, name: &str, kind: &GuestKind) -> String {
    format!("{} rollback {vmid} {name}", cli(kind))
}

/// Build `pct delsnapshot <vmid> <name>` / `qm delsnapshot <vmid> <name>`.
pub fn build_delsnapshot_command(vmid: ProxmoxVmid, name: &str, kind: &GuestKind) -> String {
    format!("{} delsnapshot {vmid} {name}", cli(kind))
}

// ─── qm list output parser ───────────────────────────────────────────────────

/// Parse the stdout of `qm list`.
///
/// Output format (whitespace-delimited):
/// ```text
///       VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID
///        100 debian-vm            running    2048               32.00 12345
///        101 ubuntu-server        stopped    1024               20.00
/// ```
///
/// Column order (0-indexed after split_whitespace):
///   col[0] = VMID, col[1] = NAME, col[2] = STATUS
///   col[3] = MEM(MB), col[4] = BOOTDISK(GB), col[5] = PID (may be absent for stopped VMs)
///
/// Parsing strategy:
///   - First line (header) is skipped.
///   - Each subsequent non-blank line is split on whitespace.
///   - Minimum 3 tokens required (VMID + NAME + STATUS).
///   - Rows where VMID fails `validate_vmid` are silently dropped (defense-in-depth).
///   - PID column is intentionally ignored.
///
/// NOTE: `qm list` has NAME before STATUS, unlike `pct list` (Status before Name).
/// This is why a separate parser is required — do NOT reuse parse_pct_list.
pub fn parse_qm_list(stdout: &str) -> Vec<VmRow> {
    let mut rows = Vec::new();
    let mut lines = stdout.lines();

    // Skip the header line.
    lines.next();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Need at least VMID + NAME + STATUS (3 tokens minimum).
        if cols.len() < 3 {
            continue;
        }
        // Validate VMID at parse source — drop row if invalid.
        let vmid = match validate_vmid(cols[0]) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // col[1] = NAME, col[2] = STATUS (qm list column order)
        let name = cols[1].to_string();
        let status = cols[2].to_string();

        rows.push(VmRow { vmid, name, status });
    }

    rows
}

// ─── pct not-available detection ─────────────────────────────────────────────

/// Heuristic: detect whether `pct` is not available on the remote host.
///
/// Returns `true` when exit_code != 0 AND stderr suggests pct is absent or
/// restricted:
///   - "command not found" — pct binary absent from PATH
///   - "not found"         — busybox/alpine variant; also "pct: not found"
///   - "permission denied" — pct requires root / sudo group
///
/// A `false` return means pct is probably available (or we got output we
/// don't understand — let the caller degrade gracefully).
pub fn is_pct_unavailable(exit_code: Option<i32>, stderr: &str) -> bool {
    if exit_code == Some(0) {
        return false;
    }
    let lower = stderr.to_lowercase();
    lower.contains("command not found")
        || lower.contains("not found")
        || lower.contains("permission denied")
}

// ─── pct list output parser ───────────────────────────────────────────────────

/// Parse the stdout of `pct list`.
///
/// Output format (whitespace-delimited):
/// ```text
/// VMID       Status     Lock         Name
/// 100        running                 debian-dev
/// 101        stopped                 ubuntu-web
/// 102        running    migrate       db-server
/// ```
///
/// Parsing strategy:
///   - First line (header) is skipped.
///   - Each subsequent line is split on whitespace.
///   - VMID = col[0], Status = col[1], Name = last token (Lock may be blank).
///   - Rows where VMID fails `validate_vmid` are silently dropped.
///   - Blank lines are skipped.
pub fn parse_pct_list(stdout: &str) -> Vec<LxcRow> {
    let mut rows = Vec::new();
    let mut lines = stdout.lines();

    // Skip the header line.
    lines.next();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Need at least VMID + Status + Name (3 tokens minimum).
        if cols.len() < 3 {
            continue;
        }
        // Validate VMID at parse source — drop row if invalid.
        let vmid = match validate_vmid(cols[0]) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let status = cols[1].to_string();
        // Name is always the last token (handles optional Lock column).
        let name = cols[cols.len() - 1].to_string();

        rows.push(LxcRow { vmid, status, name });
    }

    rows
}

// ─── pct listsnapshot output parser ──────────────────────────────────────────

/// Parse the stdout of `pct listsnapshot <vmid>`.
///
/// PVE may emit a tree-format or simple format. In practice each
/// non-header, non-current, non-arrow line contains a snapshot name as its
/// first whitespace-delimited token.
///
/// Skipped lines:
///   - Blank lines
///   - Lines starting with `->` (marks the current state)
///   - Lines whose first token is "Name" (header)
///   - Lines whose first token is "current" (the live state pseudo-snapshot)
///   - Lines with tree decorators (`+`, `|`, `\`)
///
/// Any snapshot name that fails `validate_snapshot_name` is silently dropped
/// (defense-in-depth).
pub fn parse_pct_listsnapshot(stdout: &str) -> Vec<SnapshotRow> {
    let mut rows = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Skip the current-pointer arrow lines.
        if line.starts_with("->") {
            continue;
        }
        // Split and get first token.
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.is_empty() {
            continue;
        }

        // Strip tree decorator prefix characters (+, |, \, -) if present.
        let first = cols[0].trim_start_matches(['+', '|', '\\', '-']);
        let name_candidate = if first.is_empty() && cols.len() > 1 {
            cols[1]
        } else {
            first
        };

        // Skip well-known non-snapshot tokens.
        // Known limitation: a real snapshot literally named "current", "name",
        // or "snapshots" (case-insensitive) collides with header detection and
        // will be silently dropped from the result set.
        let lower = name_candidate.to_lowercase();
        if lower == "name" || lower == "current" || lower == "snapshots" {
            continue;
        }

        // Validate at parse source.
        if validate_snapshot_name(name_candidate).is_ok() {
            rows.push(SnapshotRow {
                name: name_candidate.to_string(),
            });
        }
    }

    rows
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── WU1: validate_vmid ───────────────────────────────────────────────────

    #[test]
    fn validate_vmid_accepts_min_range() {
        assert_eq!(validate_vmid("100").unwrap(), 100);
    }

    #[test]
    fn validate_vmid_accepts_max_range() {
        assert_eq!(validate_vmid("999999999").unwrap(), 999_999_999);
    }

    #[test]
    fn validate_vmid_accepts_typical_vmid() {
        assert_eq!(validate_vmid("101").unwrap(), 101);
        assert_eq!(validate_vmid("1234").unwrap(), 1234);
    }

    #[test]
    fn validate_vmid_rejects_zero() {
        assert!(validate_vmid("0").is_err());
    }

    #[test]
    fn validate_vmid_rejects_below_minimum() {
        assert!(validate_vmid("99").is_err());
    }

    #[test]
    fn validate_vmid_rejects_empty_string() {
        assert!(validate_vmid("").is_err());
    }

    #[test]
    fn validate_vmid_rejects_alpha_chars() {
        assert!(validate_vmid("abc").is_err());
    }

    #[test]
    fn validate_vmid_rejects_injection_semicolon() {
        assert!(validate_vmid("100;rm -rf /").is_err());
    }

    #[test]
    fn validate_vmid_rejects_injection_newline() {
        assert!(validate_vmid("100\n200").is_err());
    }

    #[test]
    fn validate_vmid_rejects_u32_overflow() {
        // 10 digits, well above u32 max (4294967295 ~ 4.3B)
        assert!(validate_vmid("9999999999").is_err());
    }

    #[test]
    fn validate_vmid_rejects_999999999_plus_one() {
        // Just outside the Proxmox CTID range but fits in u32
        assert!(validate_vmid("1000000000").is_err());
    }

    #[test]
    fn validate_vmid_rejects_negative_representation() {
        // Minus sign is not a digit
        assert!(validate_vmid("-100").is_err());
    }

    #[test]
    fn validate_vmid_rejects_float() {
        assert!(validate_vmid("100.5").is_err());
    }

    // ── WU1: parse_pct_list ──────────────────────────────────────────────────

    #[test]
    fn parse_pct_list_empty_string_returns_empty_vec() {
        assert!(parse_pct_list("").is_empty());
    }

    #[test]
    fn parse_pct_list_header_only_returns_empty_vec() {
        let input = "VMID       Status     Lock         Name\n";
        assert!(parse_pct_list(input).is_empty());
    }

    #[test]
    fn parse_pct_list_single_running_row() {
        let input = "VMID       Status     Lock         Name\n100        running                 debian-dev\n";
        let rows = parse_pct_list(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 100);
        assert_eq!(rows[0].status, "running");
        assert_eq!(rows[0].name, "debian-dev");
    }

    #[test]
    fn parse_pct_list_multiple_rows() {
        let input = concat!(
            "VMID       Status     Lock         Name\n",
            "100        running                 debian-dev\n",
            "101        stopped                 ubuntu-web\n",
        );
        let rows = parse_pct_list(input);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].vmid, 100);
        assert_eq!(rows[1].status, "stopped");
        assert_eq!(rows[1].name, "ubuntu-web");
    }

    #[test]
    fn parse_pct_list_with_lock_column() {
        let input = concat!(
            "VMID       Status     Lock         Name\n",
            "102        running    migrate       db-server\n",
        );
        let rows = parse_pct_list(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 102);
        assert_eq!(rows[0].status, "running");
        assert_eq!(rows[0].name, "db-server");
    }

    #[test]
    fn parse_pct_list_skips_blank_lines() {
        let input = concat!(
            "VMID       Status     Lock         Name\n",
            "\n",
            "100        running                 myct\n",
            "\n",
        );
        let rows = parse_pct_list(input);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn parse_pct_list_drops_row_with_non_numeric_vmid() {
        let input = concat!(
            "VMID       Status     Lock         Name\n",
            "abc        running                 evil\n",
            "100        stopped                 legit\n",
        );
        let rows = parse_pct_list(input);
        // Only the valid row survives.
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 100);
    }

    #[test]
    fn parse_pct_list_drops_row_with_out_of_range_vmid() {
        let input = concat!(
            "VMID       Status     Lock         Name\n",
            "99         stopped                 toosml\n",
            "100        stopped                 legit\n",
        );
        let rows = parse_pct_list(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 100);
    }

    #[test]
    fn parse_pct_list_drops_row_with_injection_vmid() {
        let input = concat!(
            "VMID       Status     Lock         Name\n",
            "100;rm     running                 evil\n",
            "101        stopped                 legit\n",
        );
        let rows = parse_pct_list(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 101);
    }

    // ── WU2: validate_snapshot_name ──────────────────────────────────────────

    #[test]
    fn validate_snapshot_name_accepts_simple() {
        assert!(validate_snapshot_name("snap1").is_ok());
    }

    #[test]
    fn validate_snapshot_name_accepts_single_letter() {
        assert!(validate_snapshot_name("a").is_ok());
    }

    #[test]
    fn validate_snapshot_name_accepts_mixed_case_underscore_hyphen() {
        assert!(validate_snapshot_name("Snap_2-x").is_ok());
    }

    #[test]
    fn validate_snapshot_name_accepts_max_40_chars() {
        // Exactly 40 chars, starts with letter
        let name = "a".repeat(40);
        assert!(validate_snapshot_name(&name).is_ok());
    }

    #[test]
    fn validate_snapshot_name_rejects_empty() {
        assert!(validate_snapshot_name("").is_err());
    }

    #[test]
    fn validate_snapshot_name_rejects_starts_with_digit() {
        assert!(validate_snapshot_name("1starts-digit").is_err());
    }

    #[test]
    fn validate_snapshot_name_rejects_too_long() {
        // 41 chars
        let name = format!("a{}", "x".repeat(40));
        assert!(validate_snapshot_name(&name).is_err());
    }

    #[test]
    fn validate_snapshot_name_rejects_semicolon() {
        assert!(validate_snapshot_name("snap;drop").is_err());
    }

    #[test]
    fn validate_snapshot_name_rejects_space() {
        assert!(validate_snapshot_name("snap space").is_err());
    }

    #[test]
    fn validate_snapshot_name_rejects_slash() {
        assert!(validate_snapshot_name("snap/slash").is_err());
    }

    #[test]
    fn validate_snapshot_name_rejects_dot() {
        // Dots not allowed per spec
        assert!(validate_snapshot_name("snap.dot").is_err());
    }

    #[test]
    fn validate_snapshot_name_rejects_starts_with_hyphen() {
        assert!(validate_snapshot_name("-snapname").is_err());
    }

    // ── WU2: parse_pct_listsnapshot ──────────────────────────────────────────

    #[test]
    fn parse_pct_listsnapshot_empty_returns_empty() {
        assert!(parse_pct_listsnapshot("").is_empty());
    }

    #[test]
    fn parse_pct_listsnapshot_simple_format() {
        let input = "snap1\nsnap2\n";
        let rows = parse_pct_listsnapshot(input);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "snap1");
        assert_eq!(rows[1].name, "snap2");
    }

    #[test]
    fn parse_pct_listsnapshot_skips_current_line() {
        let input = concat!(
            "             Name         Snapshots\n",
            "             snap1\n",
            "->           current (no snapshot)\n",
        );
        let rows = parse_pct_listsnapshot(input);
        // "current" and "->" lines skipped; only snap1
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "snap1");
    }

    #[test]
    fn parse_pct_listsnapshot_skips_header_name_token() {
        let input = "Name    Snapshots\nsnap1\n";
        let rows = parse_pct_listsnapshot(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "snap1");
    }

    #[test]
    fn parse_pct_listsnapshot_tree_format_with_prefix() {
        // PVE tree format with +--- prefix
        let input = concat!(
            "             Name         Snapshots\n",
            "             +------- snap1 (2024-01-15 10:23:04) Description\n",
            "             +------- snap2 (2024-01-20 14:55:12) Another snap\n",
            "->           current (no snapshot)\n",
        );
        let rows = parse_pct_listsnapshot(input);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "snap1");
        assert_eq!(rows[1].name, "snap2");
    }

    #[test]
    fn parse_pct_listsnapshot_drops_invalid_snapshot_names() {
        let input = "snap1\n1invalid\nsnap2\n";
        let rows = parse_pct_listsnapshot(input);
        // "1invalid" starts with digit — dropped
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "snap1");
        assert_eq!(rows[1].name, "snap2");
    }

    // ── WU2: command builders (LXC / GuestKind::Lxc) ────────────────────────

    #[test]
    fn build_lifecycle_start() {
        assert_eq!(
            build_lifecycle_command(&LxcAction::Start, 100, &GuestKind::Lxc),
            "pct start 100"
        );
    }

    #[test]
    fn build_lifecycle_stop() {
        assert_eq!(
            build_lifecycle_command(&LxcAction::Stop, 100, &GuestKind::Lxc),
            "pct stop 100"
        );
    }

    #[test]
    fn build_lifecycle_reboot() {
        assert_eq!(
            build_lifecycle_command(&LxcAction::Reboot, 100, &GuestKind::Lxc),
            "pct reboot 100"
        );
    }

    #[test]
    fn build_listsnapshot_command_fmt() {
        assert_eq!(
            build_listsnapshot_command(101, &GuestKind::Lxc),
            "pct listsnapshot 101"
        );
    }

    #[test]
    fn build_snapshot_command_fmt() {
        assert_eq!(
            build_snapshot_command(101, "snap1", &GuestKind::Lxc),
            "pct snapshot 101 snap1"
        );
    }

    #[test]
    fn build_rollback_command_fmt() {
        assert_eq!(
            build_rollback_command(101, "snap1", &GuestKind::Lxc),
            "pct rollback 101 snap1"
        );
    }

    #[test]
    fn build_delsnapshot_command_fmt() {
        assert_eq!(
            build_delsnapshot_command(101, "snap1", &GuestKind::Lxc),
            "pct delsnapshot 101 snap1"
        );
    }

    // ── WU-VM: parse_qm_list ─────────────────────────────────────────────────

    #[test]
    fn parse_qm_list_empty_string_returns_empty_vec() {
        assert!(parse_qm_list("").is_empty());
    }

    #[test]
    fn parse_qm_list_header_only_returns_empty_vec() {
        let input = "      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID\n";
        assert!(parse_qm_list(input).is_empty());
    }

    #[test]
    fn parse_qm_list_single_running_row() {
        let input = concat!(
            "      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID\n",
            "       100 debian-vm            running    2048               32.00 12345\n",
        );
        let rows = parse_qm_list(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 100);
        assert_eq!(rows[0].name, "debian-vm");
        assert_eq!(rows[0].status, "running");
    }

    #[test]
    fn parse_qm_list_single_stopped_row_blank_pid() {
        // PID column absent for stopped VMs — only 5 tokens after split_whitespace
        let input = concat!(
            "      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID\n",
            "       101 ubuntu-server        stopped    1024               20.00\n",
        );
        let rows = parse_qm_list(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 101);
        assert_eq!(rows[0].name, "ubuntu-server");
        assert_eq!(rows[0].status, "stopped");
    }

    #[test]
    fn parse_qm_list_multiple_rows_mixed_status() {
        let input = concat!(
            "      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID\n",
            "       100 debian-vm            running    2048               32.00 12345\n",
            "       101 ubuntu-server        stopped    1024               20.00\n",
            "       200 windows-server       running    8192              100.00 99999\n",
        );
        let rows = parse_qm_list(input);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].vmid, 100);
        assert_eq!(rows[0].status, "running");
        assert_eq!(rows[1].vmid, 101);
        assert_eq!(rows[1].status, "stopped");
        assert_eq!(rows[2].vmid, 200);
        assert_eq!(rows[2].name, "windows-server");
    }

    #[test]
    fn parse_qm_list_skips_blank_lines() {
        let input = concat!(
            "      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID\n",
            "\n",
            "       100 my-vm                running    2048               32.00 1\n",
            "\n",
        );
        let rows = parse_qm_list(input);
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn parse_qm_list_drops_row_with_non_numeric_vmid() {
        let input = concat!(
            "      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID\n",
            "       abc evil-vm              running    1024               10.00 1\n",
            "       100 legit-vm             stopped    1024               10.00\n",
        );
        let rows = parse_qm_list(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 100);
    }

    #[test]
    fn parse_qm_list_drops_row_with_out_of_range_vmid() {
        let input = concat!(
            "      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID\n",
            "        99 too-small            stopped    512                8.00\n",
            "       100 legit-vm             running    1024               10.00 1\n",
        );
        let rows = parse_qm_list(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 100);
    }

    #[test]
    fn parse_qm_list_drops_row_with_injection_vmid() {
        let input = concat!(
            "      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID\n",
            "   100;rm evil-vm              running    1024               10.00 1\n",
            "       101 legit-vm             stopped    512                8.00\n",
        );
        let rows = parse_qm_list(input);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].vmid, 101);
    }

    #[test]
    fn parse_qm_list_name_is_col1_not_col_last() {
        // Contrast with pct list where Name is the LAST token.
        // For qm list, NAME is col[1] regardless of how many columns follow.
        let input = concat!(
            "      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID\n",
            "       100 my-vm                running    2048               32.00 12345\n",
        );
        let rows = parse_qm_list(input);
        assert_eq!(rows[0].name, "my-vm");
        assert_eq!(rows[0].status, "running");
    }

    // ── WU-VM: command builders (QEMU / GuestKind::Vm) ───────────────────────

    #[test]
    fn build_vm_lifecycle_start() {
        assert_eq!(
            build_lifecycle_command(&LxcAction::Start, 100, &GuestKind::Vm),
            "qm start 100"
        );
    }

    #[test]
    fn build_vm_lifecycle_stop() {
        assert_eq!(
            build_lifecycle_command(&LxcAction::Stop, 100, &GuestKind::Vm),
            "qm stop 100"
        );
    }

    #[test]
    fn build_vm_lifecycle_reboot() {
        assert_eq!(
            build_lifecycle_command(&LxcAction::Reboot, 100, &GuestKind::Vm),
            "qm reboot 100"
        );
    }

    #[test]
    fn build_vm_listsnapshot_command_fmt() {
        assert_eq!(
            build_listsnapshot_command(101, &GuestKind::Vm),
            "qm listsnapshot 101"
        );
    }

    #[test]
    fn build_vm_snapshot_command_fmt() {
        assert_eq!(
            build_snapshot_command(101, "snap1", &GuestKind::Vm),
            "qm snapshot 101 snap1"
        );
    }

    #[test]
    fn build_vm_rollback_command_fmt() {
        assert_eq!(
            build_rollback_command(101, "snap1", &GuestKind::Vm),
            "qm rollback 101 snap1"
        );
    }

    #[test]
    fn build_vm_delsnapshot_command_fmt() {
        assert_eq!(
            build_delsnapshot_command(101, "snap1", &GuestKind::Vm),
            "qm delsnapshot 101 snap1"
        );
    }

    // ── WU-VM: GuestKind::Lxc still produces pct commands ────────────────────

    #[test]
    fn lxc_kind_still_produces_pct_prefix() {
        // Regression: GuestKind::Lxc must never produce "qm ..."
        let cmd = build_lifecycle_command(&LxcAction::Start, 200, &GuestKind::Lxc);
        assert!(cmd.starts_with("pct "), "Expected pct prefix, got: {cmd}");
    }

    #[test]
    fn vm_kind_never_produces_pct_prefix() {
        // GuestKind::Vm must never produce "pct ..."
        let cmd = build_lifecycle_command(&LxcAction::Start, 200, &GuestKind::Vm);
        assert!(cmd.starts_with("qm "), "Expected qm prefix, got: {cmd}");
    }

    // ── WU3: is_pct_unavailable ──────────────────────────────────────────────

    #[test]
    fn pct_not_available_command_not_found() {
        assert!(is_pct_unavailable(Some(127), "pct: command not found\n"));
    }

    #[test]
    fn pct_not_available_not_found_busybox() {
        assert!(is_pct_unavailable(Some(1), "sh: pct: not found\n"));
    }

    #[test]
    fn pct_not_available_permission_denied() {
        assert!(is_pct_unavailable(Some(1), "permission denied\n"));
    }

    #[test]
    fn pct_not_available_pct_not_found_variant() {
        // Common on non-Proxmox hosts where the pct binary doesn't exist
        assert!(is_pct_unavailable(Some(127), "pct: not found"));
    }

    #[test]
    fn pct_not_available_false_on_exit_zero() {
        assert!(!is_pct_unavailable(Some(0), "command not found"));
    }

    #[test]
    fn pct_not_available_false_on_normal_output() {
        assert!(!is_pct_unavailable(Some(0), ""));
    }

    #[test]
    fn pct_not_available_none_exit_with_hint() {
        assert!(is_pct_unavailable(None, "pct: command not found\n"));
    }

    #[test]
    fn pct_not_available_false_on_nonzero_without_hint() {
        assert!(!is_pct_unavailable(
            Some(1),
            "Error: VM 100 is not running\n"
        ));
    }
}
