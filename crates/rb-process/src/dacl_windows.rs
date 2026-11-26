//! A DACL read back from Windows (RB-099-17): what the tests of the private folder
//! ([`crate::create_private_dir`]) and of the instance pipe
//! (`crates/rbrowser/src/instance/channel_windows.rs`) check after the system applied it. Test
//! support only: compiled for this crate's tests and, for other crates' tests, behind the
//! feature `test-support`; never in a shipped build. Win32 has no safe wrapper for the security
//! API, so this file carries the unsafe blocks, each with its SAFETY note.

use std::io;
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{LocalFree, WIN32_ERROR};
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, GetSecurityInfo, SE_FILE_OBJECT, SE_KERNEL_OBJECT,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_SIZE_INFORMATION, AclSizeInformation,
    DACL_SECURITY_INFORMATION, GetAce, GetAclInformation, GetSecurityDescriptorControl,
    PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
};

/// `ACCESS_ALLOWED_ACE_TYPE` of `winnt.h`.
pub const ACCESS_ALLOWED: u8 = 0;
/// `ACCESS_DENIED_ACE_TYPE` of `winnt.h`: the same layout as an allow entry.
pub const ACCESS_DENIED: u8 = 1;

/// One entry of a DACL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ace {
    /// The entry's type (`ACE_HEADER::AceType`), [`ACCESS_ALLOWED`] for an allow entry.
    pub kind: u8,
    /// The inheritance flags (`ACE_HEADER::AceFlags`: `OBJECT_INHERIT_ACE`, `INHERITED_ACE`, ...).
    pub flags: u8,
    /// The access mask.
    pub mask: u32,
    /// The trustee's SID as text (`S-1-...`); empty for an entry that is neither an allow nor a
    /// deny entry.
    pub sid: String,
}

/// A DACL as the system keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dacl {
    /// `SE_DACL_PROTECTED`: the parent's inheritable entries do not reach the object.
    pub protected: bool,
    /// The entries, in order.
    pub aces: Vec<Ace>,
}

/// The DACL of the file or folder at `path`.
///
/// # Errors
///
/// The object cannot be read, or it has no DACL (which would grant everyone everything).
#[allow(unsafe_code, reason = "GetNamedSecurityInfoW has no safe wrapper")]
pub fn of_path(path: &Path) -> io::Result<Dacl> {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: a NUL-terminated wide path and valid out pointers; owner, group and SACL are not
    // asked for (null). `read` frees the descriptor.
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    read(status, dacl, descriptor)
}

/// The DACL of the kernel object behind `object` (a named pipe, a job, ...); the handle needs
/// `READ_CONTROL`.
///
/// # Errors
///
/// The object cannot be read, or it has no DACL.
#[allow(unsafe_code, reason = "GetSecurityInfo has no safe wrapper")]
pub fn of_kernel_object(object: &impl AsRawHandle) -> io::Result<Dacl> {
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: the open handle `object` owns, alive for the call, and valid out pointers; owner,
    // group and SACL are not asked for (null). `read` frees the descriptor.
    let status = unsafe {
        GetSecurityInfo(
            object.as_raw_handle(),
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    read(status, dacl, descriptor)
}

/// The DACL `dacl` inside `descriptor`, as one of the two readers above returned them.
#[allow(
    unsafe_code,
    reason = "GetSecurityDescriptorControl, GetAclInformation and GetAce have no safe wrapper"
)]
fn read(status: WIN32_ERROR, dacl: *mut ACL, descriptor: PSECURITY_DESCRIPTOR) -> io::Result<Dacl> {
    if status != 0 {
        return Err(io::Error::from_raw_os_error(
            i32::try_from(status).unwrap_or(i32::MAX),
        ));
    }
    let descriptor = Descriptor(descriptor);
    if dacl.is_null() {
        return Err(io::Error::other("the object has no DACL"));
    }
    let mut control = 0_u16;
    let mut revision = 0_u32;
    // SAFETY: the live descriptor the reader returned and valid out pointers.
    if unsafe { GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut size = ACL_SIZE_INFORMATION::default();
    // SAFETY: `dacl` points into the live descriptor; `size` is a live structure of the class
    // named, with its own size.
    let ok = unsafe {
        GetAclInformation(
            dacl,
            (&raw mut size).cast(),
            u32::try_from(size_of::<ACL_SIZE_INFORMATION>()).unwrap_or(u32::MAX),
            AclSizeInformation,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut aces = Vec::new();
    for index in 0..size.AceCount {
        let mut ace: *mut core::ffi::c_void = null_mut();
        // SAFETY: `dacl` points into the live descriptor, `index` is below its entry count and
        // `ace` is a valid out pointer; the entry points into the DACL.
        if unsafe { GetAce(dacl, index, &mut ace) } == 0 || ace.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: every entry starts with an ACE_HEADER; unaligned reads ask nothing of the
        // entry's place in the DACL.
        let header = unsafe { ace.cast::<ACE_HEADER>().read_unaligned() };
        // SAFETY: every entry type of a DACL carries its access mask right after the header.
        let mask = unsafe { entry_mask(ace) };
        let sid = if header.AceType == ACCESS_ALLOWED || header.AceType == ACCESS_DENIED {
            // SAFETY: an allow or deny entry has the ACCESS_ALLOWED_ACE layout: its SID starts at
            // `SidStart` and runs to the entry's end, inside the live DACL.
            unsafe {
                crate::user_sid::platform::sid_text(
                    ace.cast::<u8>()
                        .add(std::mem::offset_of!(ACCESS_ALLOWED_ACE, SidStart))
                        .cast(),
                )
            }?
        } else {
            String::new()
        };
        aces.push(Ace {
            kind: header.AceType,
            flags: header.AceFlags,
            mask,
            sid,
        });
    }
    Ok(Dacl {
        protected: control & SE_DACL_PROTECTED != 0,
        aces,
    })
}

/// The access mask right after the header of the entry at `ace`.
///
/// # Safety
///
/// `ace` points to a live entry of at least `ACE_HEADER` plus four bytes.
#[allow(unsafe_code, reason = "reads the mask behind a raw pointer")]
unsafe fn entry_mask(ace: *mut core::ffi::c_void) -> u32 {
    // SAFETY: the caller's promise: the four bytes after the header are inside the entry.
    unsafe {
        ace.cast::<u8>()
            .add(size_of::<ACE_HEADER>())
            .cast::<u32>()
            .read_unaligned()
    }
}

/// A security descriptor the system allocated, freed on drop.
struct Descriptor(PSECURITY_DESCRIPTOR);

impl Drop for Descriptor {
    #[allow(
        unsafe_code,
        reason = "LocalFree releases what GetNamedSecurityInfoW or GetSecurityInfo allocated"
    )]
    fn drop(&mut self) {
        // SAFETY: allocated by the reader with LocalAlloc, freed once.
        unsafe { LocalFree(self.0) };
    }
}
