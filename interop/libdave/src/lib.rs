//! A safe wrapper of the libdave C API (`dave.h`) for the interoperability tests of cordial.
//!
//! libdave is the C++ DAVE library of Discord (<https://github.com/discord/libdave>). The wrapper
//! loads the libdave shared library at run time from the path in the `LIBDAVE_PATH` environment
//! variable. It wraps only the functions that the tests use, for Opus audio.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::io::Write as _;
use std::sync::{Mutex, OnceLock};

use libloading::Library;

/// The environment variable with the path of the libdave shared library.
pub const PATH_VARIABLE: &str = "LIBDAVE_PATH";

/// The DAVE protocol version of the tests.
const PROTOCOL_VERSION: u16 = 1;

/// The SSRC that the encryptor uses. libdave selects the codec of a frame from its SSRC.
const SSRC: u32 = 1;

/// `DAVE_MEDIA_TYPE_AUDIO`.
const MEDIA_TYPE_AUDIO: c_int = 0;
/// `DAVE_CODEC_OPUS`.
const CODEC_OPUS: c_int = 1;
/// `DAVE_ENCRYPTOR_RESULT_CODE_SUCCESS` and `DAVE_DECRYPTOR_RESULT_CODE_SUCCESS`.
const RESULT_SUCCESS: c_int = 0;
/// `DAVE_LOGGING_SEVERITY_WARNING`.
const LOG_WARNING: c_int = 2;

type Handle = *mut c_void;
type FailureCallback = unsafe extern "C" fn(*const c_char, *const c_char, *mut c_void);
type LogSink = unsafe extern "C" fn(c_int, *const c_char, c_int, *const c_char);

/// The libdave functions. Each field has the signature of the function in `dave.h`.
struct Api {
    max_supported_protocol_version: unsafe extern "C" fn() -> u16,
    free: unsafe extern "C" fn(*mut c_void),
    set_log_sink: unsafe extern "C" fn(Option<LogSink>),

    session_create: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        Option<FailureCallback>,
        *mut c_void,
    ) -> Handle,
    session_destroy: unsafe extern "C" fn(Handle),
    session_init: unsafe extern "C" fn(Handle, u16, u64, *const c_char),
    session_set_external_sender: unsafe extern "C" fn(Handle, *const u8, usize),
    session_key_package: unsafe extern "C" fn(Handle, *mut *mut u8, *mut usize),
    session_process_proposals: unsafe extern "C" fn(
        Handle,
        *const u8,
        usize,
        *const *const c_char,
        usize,
        *mut *mut u8,
        *mut usize,
    ),
    session_process_commit: unsafe extern "C" fn(Handle, *const u8, usize) -> Handle,
    session_process_welcome:
        unsafe extern "C" fn(Handle, *const u8, usize, *const *const c_char, usize) -> Handle,
    session_key_ratchet: unsafe extern "C" fn(Handle, *const c_char) -> Handle,

    commit_result_is_failed: unsafe extern "C" fn(Handle) -> bool,
    commit_result_is_ignored: unsafe extern "C" fn(Handle) -> bool,
    commit_result_roster: unsafe extern "C" fn(Handle, *mut *mut u64, *mut usize),
    commit_result_destroy: unsafe extern "C" fn(Handle),
    welcome_result_roster: unsafe extern "C" fn(Handle, *mut *mut u64, *mut usize),
    welcome_result_destroy: unsafe extern "C" fn(Handle),
    key_ratchet_destroy: unsafe extern "C" fn(Handle),

    encryptor_create: unsafe extern "C" fn() -> Handle,
    encryptor_destroy: unsafe extern "C" fn(Handle),
    encryptor_set_key_ratchet: unsafe extern "C" fn(Handle, Handle),
    encryptor_set_passthrough: unsafe extern "C" fn(Handle, bool),
    encryptor_assign_ssrc: unsafe extern "C" fn(Handle, u32, c_int),
    encryptor_max_size: unsafe extern "C" fn(Handle, c_int, usize) -> usize,
    encryptor_encrypt: unsafe extern "C" fn(
        Handle,
        c_int,
        u32,
        *const u8,
        usize,
        *mut u8,
        usize,
        *mut usize,
    ) -> c_int,

    decryptor_create: unsafe extern "C" fn() -> Handle,
    decryptor_destroy: unsafe extern "C" fn(Handle),
    decryptor_set_key_ratchet: unsafe extern "C" fn(Handle, Handle),
    decryptor_max_size: unsafe extern "C" fn(Handle, c_int, usize) -> usize,
    decryptor_decrypt:
        unsafe extern "C" fn(Handle, c_int, *const u8, usize, *mut u8, usize, *mut usize) -> c_int,

    /// Keeps the library loaded while the function pointers exist.
    _library: Library,
}

static API: OnceLock<Api> = OnceLock::new();

/// Gives the loaded libdave functions. The first call loads the library.
fn api() -> &'static Api {
    API.get_or_init(load)
}

fn load() -> Api {
    let path = std::env::var_os(PATH_VARIABLE)
        .unwrap_or_else(|| panic!("set {PATH_VARIABLE} to the libdave shared library"));
    // SAFETY: libdave has no initialization code that changes the state of this process.
    let library = unsafe { Library::new(&path) }
        .unwrap_or_else(|error| panic!("cannot load {}: {error}", path.display()));

    // SAFETY: the type of each field is the signature of the function in `dave.h`.
    let api = unsafe {
        Api {
            max_supported_protocol_version: function(&library, "daveMaxSupportedProtocolVersion"),
            free: function(&library, "daveFree"),
            set_log_sink: function(&library, "daveSetLogSinkCallback"),
            session_create: function(&library, "daveSessionCreate"),
            session_destroy: function(&library, "daveSessionDestroy"),
            session_init: function(&library, "daveSessionInit"),
            session_set_external_sender: function(&library, "daveSessionSetExternalSender"),
            session_key_package: function(&library, "daveSessionGetMarshalledKeyPackage"),
            session_process_proposals: function(&library, "daveSessionProcessProposals"),
            session_process_commit: function(&library, "daveSessionProcessCommit"),
            session_process_welcome: function(&library, "daveSessionProcessWelcome"),
            session_key_ratchet: function(&library, "daveSessionGetKeyRatchet"),
            commit_result_is_failed: function(&library, "daveCommitResultIsFailed"),
            commit_result_is_ignored: function(&library, "daveCommitResultIsIgnored"),
            commit_result_roster: function(&library, "daveCommitResultGetRosterMemberIds"),
            commit_result_destroy: function(&library, "daveCommitResultDestroy"),
            welcome_result_roster: function(&library, "daveWelcomeResultGetRosterMemberIds"),
            welcome_result_destroy: function(&library, "daveWelcomeResultDestroy"),
            key_ratchet_destroy: function(&library, "daveKeyRatchetDestroy"),
            encryptor_create: function(&library, "daveEncryptorCreate"),
            encryptor_destroy: function(&library, "daveEncryptorDestroy"),
            encryptor_set_key_ratchet: function(&library, "daveEncryptorSetKeyRatchet"),
            encryptor_set_passthrough: function(&library, "daveEncryptorSetPassthroughMode"),
            encryptor_assign_ssrc: function(&library, "daveEncryptorAssignSsrcToCodec"),
            encryptor_max_size: function(&library, "daveEncryptorGetMaxCiphertextByteSize"),
            encryptor_encrypt: function(&library, "daveEncryptorEncrypt"),
            decryptor_create: function(&library, "daveDecryptorCreate"),
            decryptor_destroy: function(&library, "daveDecryptorDestroy"),
            decryptor_set_key_ratchet: function(&library, "daveDecryptorTransitionToKeyRatchet"),
            decryptor_max_size: function(&library, "daveDecryptorGetMaxPlaintextByteSize"),
            decryptor_decrypt: function(&library, "daveDecryptorDecrypt"),
            _library: library,
        }
    };

    // SAFETY: `log_sink` has the signature of `DAVELogSinkCallback` and does not unwind.
    unsafe { (api.set_log_sink)(Some(log_sink)) };
    api
}

/// Gives the function with a name from the library.
///
/// # Safety
///
/// `T` must be the function pointer type of the function in `dave.h`.
unsafe fn function<T: Copy>(library: &Library, name: &str) -> T {
    let symbol_name = format!("{name}\0");
    // SAFETY: the caller gives the correct type.
    let symbol = unsafe { library.get::<T>(symbol_name.as_bytes()) }
        .unwrap_or_else(|error| panic!("libdave has no function `{name}`: {error}"));
    *symbol
}

/// Copies a C string. A null pointer gives an empty string.
///
/// # Safety
///
/// `text` must be null or a valid C string.
unsafe fn c_text(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: the caller gives a valid C string.
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

/// Writes the warnings and errors of libdave to the standard error. It ignores the other levels,
/// because libdave logs each frame at the verbose level.
unsafe extern "C" fn log_sink(
    severity: c_int,
    _file: *const c_char,
    _line: c_int,
    message: *const c_char,
) {
    if severity < LOG_WARNING {
        return;
    }
    // SAFETY: libdave gives a valid C string that stays valid during the call.
    let message = unsafe { c_text(message) };
    // A panic must not unwind into C++. Thus a write error is ignored.
    let _ = writeln!(std::io::stderr().lock(), "libdave: {message}");
}

/// Records an MLS failure in the failure list of a session.
unsafe extern "C" fn record_failure(
    source: *const c_char,
    reason: *const c_char,
    user_data: *mut c_void,
) {
    // SAFETY: `user_data` is the failure list of the session, which the session keeps until
    // after `daveSessionDestroy`.
    let failures = unsafe { &*user_data.cast::<Mutex<Vec<String>>>() };
    // SAFETY: libdave gives valid C strings that stay valid during the call.
    let text = unsafe { format!("{}: {}", c_text(source), c_text(reason)) };
    if let Ok(mut failures) = failures.lock() {
        failures.push(text);
    }
}

/// Gives the highest DAVE protocol version of libdave.
#[must_use]
pub fn max_supported_protocol_version() -> u16 {
    // SAFETY: the function has no arguments.
    unsafe { (api().max_supported_protocol_version)() }
}

/// Copies a byte buffer of libdave and frees it.
///
/// # Safety
///
/// `data` must be null or a buffer of `length` bytes that libdave allocated.
unsafe fn take_bytes(data: *mut u8, length: usize) -> Vec<u8> {
    if data.is_null() {
        return Vec::new();
    }
    // SAFETY: the caller gives a valid buffer of `length` bytes.
    let bytes = unsafe { std::slice::from_raw_parts(data, length) }.to_vec();
    // SAFETY: libdave allocated the buffer, and nothing uses it after this call.
    unsafe { (api().free)(data.cast()) };
    bytes
}

/// Copies an ID array of libdave and frees it.
///
/// # Safety
///
/// `data` must be null or an array of `length` IDs that libdave allocated.
unsafe fn take_ids(data: *mut u64, length: usize) -> Vec<u64> {
    if data.is_null() {
        return Vec::new();
    }
    // SAFETY: the caller gives a valid array of `length` IDs.
    let ids = unsafe { std::slice::from_raw_parts(data, length) }.to_vec();
    // SAFETY: libdave allocated the array, and nothing uses it after this call.
    unsafe { (api().free)(data.cast()) };
    ids
}

/// The user IDs as C strings, and the pointer array that libdave reads.
struct UserIds {
    _texts: Vec<CString>,
    pointers: Vec<*const c_char>,
}

impl UserIds {
    fn new(user_ids: &[u64]) -> Self {
        let texts: Vec<CString> = user_ids.iter().map(|&id| user_id_text(id)).collect();
        let pointers = texts.iter().map(|text| text.as_ptr()).collect();
        Self {
            _texts: texts,
            pointers,
        }
    }
}

fn user_id_text(user_id: u64) -> CString {
    CString::new(user_id.to_string()).expect("a decimal number has no NUL byte")
}

/// The result of a commit (opcode 29) in libdave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitResult {
    /// The commit is not valid.
    Failed,
    /// The session ignored the commit, for example because it has no group.
    Ignored,
    /// The session applied the commit. The roster gives the IDs of the added and removed users.
    Applied(Vec<u64>),
}

/// A libdave MLS session (`DAVESessionHandle`).
pub struct Session {
    handle: Handle,
    /// The MLS failures that libdave reported. libdave keeps a pointer to the list.
    failures: Box<Mutex<Vec<String>>>,
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("failures", &self.failures())
            .finish_non_exhaustive()
    }
}

impl Session {
    /// Makes a session and starts it with protocol version 1 for a user in a channel.
    ///
    /// # Panics
    ///
    /// Panics when libdave cannot make the session.
    #[must_use]
    pub fn new(user_id: u64, channel_id: u64) -> Self {
        let api = api();
        let failures = Box::new(Mutex::new(Vec::new()));
        let user_data: *const Mutex<Vec<String>> = &raw const *failures;
        // SAFETY: the callback has the signature of `DAVEMLSFailureCallback`. The failure list
        // lives in a box that the session keeps until after `daveSessionDestroy`.
        let handle = unsafe {
            (api.session_create)(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(record_failure),
                user_data.cast_mut().cast(),
            )
        };
        assert!(!handle.is_null(), "libdave did not make a session");

        let user_id = user_id_text(user_id);
        // SAFETY: the handle is valid, and the user ID is a valid C string during the call.
        unsafe { (api.session_init)(handle, PROTOCOL_VERSION, channel_id, user_id.as_ptr()) };
        Self { handle, failures }
    }

    /// Sets the external sender from the payload of opcode 25.
    pub fn set_external_sender(&self, payload: &[u8]) {
        // SAFETY: the handle and the payload are valid during the call.
        unsafe {
            (api().session_set_external_sender)(self.handle, payload.as_ptr(), payload.len())
        };
    }

    /// Gives the key package of the session: the payload of opcode 26.
    #[must_use]
    pub fn key_package(&self) -> Vec<u8> {
        let mut data = std::ptr::null_mut();
        let mut length = 0;
        // SAFETY: the handle is valid, and the output pointers point to local values.
        unsafe { (api().session_key_package)(self.handle, &raw mut data, &raw mut length) };
        // SAFETY: libdave allocated the buffer with this length.
        unsafe { take_bytes(data, length) }
    }

    /// Handles the payload of opcode 27. Gives the payload of opcode 28 (a commit and an optional
    /// Welcome), or `None` when there is nothing to commit or the proposals are not valid.
    #[must_use]
    pub fn process_proposals(&self, payload: &[u8], recognized: &[u64]) -> Option<Vec<u8>> {
        let recognized = UserIds::new(recognized);
        let mut data = std::ptr::null_mut();
        let mut length = 0;
        // SAFETY: the handle, the payload, and the ID array are valid during the call. The
        // output pointers point to local values.
        unsafe {
            (api().session_process_proposals)(
                self.handle,
                payload.as_ptr(),
                payload.len(),
                recognized.pointers.as_ptr(),
                recognized.pointers.len(),
                &raw mut data,
                &raw mut length,
            );
        }
        // SAFETY: libdave allocated the buffer with this length.
        let commit_welcome = unsafe { take_bytes(data, length) };
        (!commit_welcome.is_empty()).then_some(commit_welcome)
    }

    /// Handles a commit of opcode 29, without the transition ID.
    #[must_use]
    pub fn process_commit(&self, commit: &[u8]) -> CommitResult {
        let api = api();
        // SAFETY: the handle and the commit are valid during the call.
        let result =
            unsafe { (api.session_process_commit)(self.handle, commit.as_ptr(), commit.len()) };
        if result.is_null() {
            return CommitResult::Failed;
        }

        // SAFETY: the result handle is valid until `daveCommitResultDestroy`.
        let outcome = unsafe {
            if (api.commit_result_is_failed)(result) {
                CommitResult::Failed
            } else if (api.commit_result_is_ignored)(result) {
                CommitResult::Ignored
            } else {
                let mut data = std::ptr::null_mut();
                let mut length = 0;
                (api.commit_result_roster)(result, &raw mut data, &raw mut length);
                CommitResult::Applied(take_ids(data, length))
            }
        };
        // SAFETY: nothing uses the result handle after this call.
        unsafe { (api.commit_result_destroy)(result) };
        outcome
    }

    /// Handles a Welcome of opcode 30, without the transition ID. Gives the roster IDs, or
    /// `None` when the Welcome is not valid.
    #[must_use]
    pub fn process_welcome(&self, welcome: &[u8], recognized: &[u64]) -> Option<Vec<u64>> {
        let api = api();
        let recognized = UserIds::new(recognized);
        // SAFETY: the handle, the Welcome, and the ID array are valid during the call.
        let result = unsafe {
            (api.session_process_welcome)(
                self.handle,
                welcome.as_ptr(),
                welcome.len(),
                recognized.pointers.as_ptr(),
                recognized.pointers.len(),
            )
        };
        if result.is_null() {
            return None;
        }

        let mut data = std::ptr::null_mut();
        let mut length = 0;
        // SAFETY: the result handle is valid, and the output pointers point to local values.
        let roster = unsafe {
            (api.welcome_result_roster)(result, &raw mut data, &raw mut length);
            take_ids(data, length)
        };
        // SAFETY: nothing uses the result handle after this call.
        unsafe { (api.welcome_result_destroy)(result) };
        Some(roster)
    }

    /// Gives the key ratchet of a user in the current epoch, or `None` when the user is not in
    /// the group.
    #[must_use]
    pub fn key_ratchet(&self, user_id: u64) -> Option<KeyRatchet> {
        let user_id = user_id_text(user_id);
        // SAFETY: the handle and the user ID are valid during the call.
        let handle = unsafe { (api().session_key_ratchet)(self.handle, user_id.as_ptr()) };
        (!handle.is_null()).then_some(KeyRatchet { handle })
    }

    /// Gives the MLS failures that libdave reported for this session.
    #[must_use]
    pub fn failures(&self) -> Vec<String> {
        self.failures
            .lock()
            .map(|failures| failures.clone())
            .unwrap_or_default()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: nothing uses the handle after this call. The failure list drops after it.
        unsafe { (api().session_destroy)(self.handle) };
    }
}

/// A key ratchet of one sender in one epoch (`DAVEKeyRatchetHandle`).
pub struct KeyRatchet {
    handle: Handle,
}

impl fmt::Debug for KeyRatchet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyRatchet").finish_non_exhaustive()
    }
}

impl Drop for KeyRatchet {
    fn drop(&mut self) {
        // SAFETY: nothing uses the handle after this call. The encryptor and the decryptor keep
        // their own copies.
        unsafe { (api().key_ratchet_destroy)(self.handle) };
    }
}

/// A libdave frame encryptor for Opus audio (`DAVEEncryptorHandle`).
pub struct Encryptor {
    handle: Handle,
}

impl fmt::Debug for Encryptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Encryptor").finish_non_exhaustive()
    }
}

impl Default for Encryptor {
    fn default() -> Self {
        Self::new()
    }
}

impl Encryptor {
    /// Makes an encryptor with no key ratchet and no passthrough.
    ///
    /// # Panics
    ///
    /// Panics when libdave cannot make the encryptor.
    #[must_use]
    pub fn new() -> Self {
        let api = api();
        // SAFETY: the function has no arguments.
        let handle = unsafe { (api.encryptor_create)() };
        assert!(!handle.is_null(), "libdave did not make an encryptor");
        // SAFETY: the handle is valid.
        unsafe {
            (api.encryptor_assign_ssrc)(handle, SSRC, CODEC_OPUS);
            (api.encryptor_set_passthrough)(handle, false);
        }
        Self { handle }
    }

    /// Sets the key ratchet of the own user. The encryptor keeps a copy.
    pub fn set_key_ratchet(&mut self, ratchet: &KeyRatchet) {
        // SAFETY: both handles are valid. libdave copies the key ratchet.
        unsafe { (api().encryptor_set_key_ratchet)(self.handle, ratchet.handle) };
    }

    /// Encrypts an Opus frame. Gives the libdave result code on a failure.
    ///
    /// # Errors
    ///
    /// Gives the `DAVEEncryptorResultCode` when the encryption fails.
    pub fn encrypt(&mut self, frame: &[u8]) -> Result<Vec<u8>, i32> {
        let api = api();
        // SAFETY: the handle is valid.
        let capacity =
            unsafe { (api.encryptor_max_size)(self.handle, MEDIA_TYPE_AUDIO, frame.len()) };
        let mut output = vec![0; capacity];
        let mut written = 0;
        // SAFETY: the handle, the frame, and the output buffer are valid during the call. The
        // output buffer has the capacity that libdave gave.
        let code = unsafe {
            (api.encryptor_encrypt)(
                self.handle,
                MEDIA_TYPE_AUDIO,
                SSRC,
                frame.as_ptr(),
                frame.len(),
                output.as_mut_ptr(),
                output.len(),
                &raw mut written,
            )
        };
        if code != RESULT_SUCCESS {
            return Err(code);
        }
        output.truncate(written);
        Ok(output)
    }
}

impl Drop for Encryptor {
    fn drop(&mut self) {
        // SAFETY: nothing uses the handle after this call.
        unsafe { (api().encryptor_destroy)(self.handle) };
    }
}

/// A libdave frame decryptor for one sender (`DAVEDecryptorHandle`).
pub struct Decryptor {
    handle: Handle,
}

impl fmt::Debug for Decryptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Decryptor").finish_non_exhaustive()
    }
}

impl Default for Decryptor {
    fn default() -> Self {
        Self::new()
    }
}

impl Decryptor {
    /// Makes a decryptor with no key ratchet.
    ///
    /// # Panics
    ///
    /// Panics when libdave cannot make the decryptor.
    #[must_use]
    pub fn new() -> Self {
        // SAFETY: the function has no arguments.
        let handle = unsafe { (api().decryptor_create)() };
        assert!(!handle.is_null(), "libdave did not make a decryptor");
        Self { handle }
    }

    /// Changes to the key ratchet of a new epoch. The old keys stay usable for a short time.
    pub fn set_key_ratchet(&mut self, ratchet: &KeyRatchet) {
        // SAFETY: both handles are valid. libdave copies the key ratchet.
        unsafe { (api().decryptor_set_key_ratchet)(self.handle, ratchet.handle) };
    }

    /// Decrypts a protocol frame.
    ///
    /// # Errors
    ///
    /// Gives the `DAVEDecryptorResultCode` when the decryption fails.
    pub fn decrypt(&mut self, frame: &[u8]) -> Result<Vec<u8>, i32> {
        let api = api();
        // SAFETY: the handle is valid.
        let capacity =
            unsafe { (api.decryptor_max_size)(self.handle, MEDIA_TYPE_AUDIO, frame.len()) };
        let mut output = vec![0; capacity];
        let mut written = 0;
        // SAFETY: the handle, the frame, and the output buffer are valid during the call. The
        // output buffer has the capacity that libdave gave.
        let code = unsafe {
            (api.decryptor_decrypt)(
                self.handle,
                MEDIA_TYPE_AUDIO,
                frame.as_ptr(),
                frame.len(),
                output.as_mut_ptr(),
                output.len(),
                &raw mut written,
            )
        };
        if code != RESULT_SUCCESS {
            return Err(code);
        }
        output.truncate(written);
        Ok(output)
    }
}

impl Drop for Decryptor {
    fn drop(&mut self) {
        // SAFETY: nothing uses the handle after this call.
        unsafe { (api().decryptor_destroy)(self.handle) };
    }
}
