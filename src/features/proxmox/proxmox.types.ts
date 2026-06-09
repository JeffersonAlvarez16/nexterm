// features/proxmox/proxmox.types.ts — Proxmox type aliases
//
// Re-exports store types for use within the proxmox feature module.

export type {
  LxcRow,
  VmRow,
  SnapshotRow,
  LxcAction,
  GuestKind,
} from "../../stores/proxmoxStore";
