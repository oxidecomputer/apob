use strum::FromRepr;
use zerocopy::FromBytes;

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

#[derive(Copy, Clone, Debug, FromBytes)]
pub struct ApobHeader {
    pub sig: [u8; 4],
    pub version: u32,
    pub size: u32,
    pub offset: u32,
}

const APOB_HMAC_LEN: usize = 32;

#[derive(Copy, Clone, Debug, FromBytes)]
pub struct ApobEntry {
    pub group: u32,
    pub ty: u32,
    pub inst: u32,

    /// Size in bytes of this struct, including the header
    pub size: u32,
    pub hmac: [u8; APOB_HMAC_LEN],
    // data is trailing behind here
}

/// Signature, which must be the first 4 bytes of the blob
pub const APOB_SIG: [u8; 4] = *b"APOB";
