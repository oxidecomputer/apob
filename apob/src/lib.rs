#![no_std]

use strum_macros::FromRepr;
use zerocopy::{FromBytes, Immutable, KnownLayout};

/// Signature, which must be the first 4 bytes of the blob
pub const APOB_SIG: [u8; 4] = *b"APOB";

/// Known version
pub const APOB_VERSION: u32 = 0x18;

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct ApobHeader {
    pub sig: [u8; 4],
    pub version: u32,
    pub size: u32,
    pub offset: u32,
}

////////////////////////////////////////////////////////////////////////////////

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

/// Mask applied to [`ApobEntry::group`] to cancel the group
pub const APOB_CANCELLED: u32 = 0xFFFF_0000;
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

////////////////////////////////////////////////////////////////////////////////
// GENERAL group handling

#[derive(Copy, Clone, Debug, FromRepr)]
#[allow(non_camel_case_types)]
pub enum ApobGeneralType {
    EVENT_LOG = 6,
}

/// [`ApobGroup::GENERAL`] + [`ApobGeneralType::EVENT_LOG`]
#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct MilanApobEventLog {
    pub count: u16,
    _pad: u16,
    pub events: [MilanApobEvent; 64],
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct MilanApobEvent {
    pub class: u32,
    pub info: u32,
    pub data0: u32,
    pub data1: u32,
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

#[derive(Copy, Clone, Debug, FromRepr)]
#[allow(non_camel_case_types)]
pub enum MilanApobEventInfo {
    TRAIN_ERROR = 0x4001,
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct MilanTrainErrorData0(pub u32);

impl MilanTrainErrorData0 {
    pub fn sock(&self) -> u32 {
        self.0 & 0xFF
    }
    pub fn chan(&self) -> u32 {
        (self.0 >> 8) & 0xFF
    }
    pub fn dimm(&self) -> u32 {
        (self.0 >> 16) & 0b11
    }
    pub fn rank(&self) -> u32 {
        (self.0 >> 24) & 0b1111
    }
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct MilanTrainErrorData1(pub u32);

impl MilanTrainErrorData1 {
    pub fn pmu_load(&self) -> bool {
        (self.0 & 1) != 0
    }
    pub fn pmu_train(&self) -> bool {
        (self.0 & 2) != 0
    }
}

////////////////////////////////////////////////////////////////////////////////
// FABRIC group handling

#[derive(Copy, Clone, Debug, FromRepr)]
#[allow(non_camel_case_types)]
pub enum ApobFabricType {
    SYS_MEM_MAP = 9,
    MILAN_FABRIC_PHY_OVERRIDE = 21,
}

const MILAN_APOB_CCX_MAX_CCDS: usize = 8;
const MILAN_APOB_CCX_MAX_CCXS: usize = 2;
const MILAN_APOB_CCX_MAX_CORES: usize = 8;
const MILAN_APOB_CCX_MAX_THREADS: usize = 2;

/// [`ApobGroup::FABRIC`] + [`ApobFabricType::SYS_MEM_MAP`]
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

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C, packed)]
pub struct MilanApobCoremap {
    pub ccds: [MilanApobCcd; MILAN_APOB_CCX_MAX_CCDS],
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C, packed)]
pub struct MilanApobCcd {
    pub macd_id: u8,
    pub macd_ccxs: [MilanApobCcx; MILAN_APOB_CCX_MAX_CCXS],
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C, packed)]
pub struct MilanApobCcx {
    pub macx_id: u8,
    pub macx_cores: [MilanApobCore; MILAN_APOB_CCX_MAX_CORES],
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C, packed)]
pub struct MilanApobCore {
    pub mac_id: u8,
    pub mac_thread_exists: [u8; MILAN_APOB_CCX_MAX_THREADS],
}

/// [`ApobGroup::FABRIC`] + [`ApobFabricType::MILAN_FABRIC_PHY_OVERRIDE`]
#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C, packed)]
pub struct MilanApobPhyOverride {
    pub map_datalen: u32,
    pub map_data: [u8; 256],
}

////////////////////////////////////////////////////////////////////////////////
// MEMORY group

#[derive(Copy, Clone, Debug, FromRepr)]
#[allow(non_camel_case_types)]
pub enum ApobMemoryType {
    MILAN_PMU_TRAIN_FAIL = 22,
}

#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct PmuTfiEntryBitfield(pub u32);

impl PmuTfiEntryBitfield {
    pub fn sock(&self) -> u32 {
        self.0 & 1
    }
    pub fn umc(&self) -> u32 {
        (self.0 >> 1) & 0b111
    }
    /// 0 for 1D and 1 for 2D
    pub fn dimension(&self) -> u32 {
        (self.0 >> 4) & 1
    }

    pub fn num_1d(&self) -> u32 {
        (self.0 >> 5) & 0b111
    }

    pub fn stage(&self) -> u32 {
        (self.0 >> 15) & 0xFFFF
    }
}

/// A single training error entry
#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct PmuTfiEntry {
    pub bits: PmuTfiEntryBitfield,
    pub error: u32,
    pub data: [u32; 4],
}

/// A set of training failure entries
#[derive(Copy, Clone, Debug, FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct PmuTfi {
    /// Position of the next valid entry
    pub nvalid: u32,
    pub entries: [PmuTfiEntry; 40],
}
