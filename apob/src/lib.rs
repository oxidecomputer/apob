#![no_std]

use strum::FromRepr;
use zerocopy::{FromBytes, Immutable, KnownLayout};

#[derive(Copy, Clone, Debug, FromRepr)]
#[allow(non_camel_case_types)]
pub enum ApobGroup {
    MEMORY = 1,
    DF,
    CCX,
    NBIO,
    FCH,
    PSP,
    GENERAL,
    SMBIOS,
    FABRIC,
    APCB,
}

#[derive(Copy, Clone, Debug, FromRepr)]
#[allow(non_camel_case_types)]
pub enum ApobFabricType {
    SYS_MEM_MAP = 9,
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct ApobHeader {
    pub sig: [u8; 4],
    pub version: u32,
    pub size: u32,
    pub offset: u32,
}

const APOB_HMAC_LEN: usize = 32;

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct ApobEntry {
    pub group: u32,
    pub ty: u32,
    pub inst: u32,

    /// Size in bytes of this struct, including the header
    pub size: u32,
    pub hmac: [u8; APOB_HMAC_LEN],
    // data is trailing behind here
}

impl ApobEntry {
    /// Returns the group, or `None` if the type is unknown
    pub fn group(&self) -> Option<ApobGroup> {
        let group = self.group & !APOB_CANCELLED;
        ApobGroup::from_repr(group as usize)
    }
    /// Checks whether this group has been cancelled
    ///
    /// A group is cancelled when its top 16 bits are all set to 1
    pub fn cancelled(&self) -> bool {
        (self.group & APOB_CANCELLED) == APOB_CANCELLED
    }
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct ApobSysMemMap {
    /// Physical address of the upper limit (exclusive) of available RAM
    pub high_phys: u64,

    /// Number of [`ApobSysMemMapHole`] entries following this structure
    pub hole_count: u32,
    _padding: u32,
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct ApobSysMemMapHole {
    /// Base physical address of this hole
    pub base: u64,

    /// Size of the hole in bytes
    pub size: u64,

    /// Tag indicating the purpose of this hole
    ///
    /// The specific values may vary between different microarchitectures and/or
    /// firmware.
    pub ty: u32,
    _padding: u32,
}

/// Signature, which must be the first 4 bytes of the blob
pub const APOB_SIG: [u8; 4] = *b"APOB";

/// Known version
pub const APOB_VERSION: u32 = 0x18;

/// Mask applied to [`ApobEntry::group`] to cancel the group
pub const APOB_CANCELLED: u32 = 0xFFFF_0000;

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct MilanApobEvent {
    pub class: u32,
    pub info: u32,
    pub data0: u32,
    pub data1: u32,
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct MilanApobEventLog {
    pub count: u16,
    _pad: u16,
    pub events: [MilanApobEvent; 64],
}

#[derive(Copy, Clone, Debug, FromRepr)]
#[allow(non_camel_case_types)]
pub enum MilanApobEventClass {
    ALERT = 5,
    WARN = 6,
    ERROR = 7,
    CRIT = 8,
    FATAL = 9,
}

const MILAN_APOB_CCX_MAX_THREADS: usize = 2;

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(packed)]
pub struct MilanApobCore {
    pub mac_id: u8,
    pub mac_thread_exists: [u8; MILAN_APOB_CCX_MAX_THREADS],
}

const MILAN_APOB_CCX_MAX_CORES: usize = 8;

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(packed)]
pub struct MilanApobCcx {
    pub macx_id: u8,
    pub macx_cores: [MilanApobCore; MILAN_APOB_CCX_MAX_CORES],
}

const MILAN_APOB_CCX_MAX_CCXS: usize = 2;

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(packed)]
pub struct MilanApobCcd {
    pub macd_id: u8,
    pub macd_ccxs: [MilanApobCcx; MILAN_APOB_CCX_MAX_CCXS],
}

const MILAN_APOB_CCX_MAX_CCDS: usize = 8;

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(packed)]
pub struct MilanApobCoremap {
    pub ccds: [MilanApobCcd; MILAN_APOB_CCX_MAX_CCDS],
}
