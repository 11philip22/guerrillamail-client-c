use guerrillamail_client::{Client, ClientBuilder, EmailDetails, Error, Message};
use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::mem;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::time::Duration;
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

thread_local! {
    static LAST_ERROR: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

#[repr(C)]
pub struct gm_builder_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct gm_client_t {
    _private: [u8; 0],
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum gm_status_t {
    GM_OK = 0,
    GM_ERR_NULL = 1,
    GM_ERR_INVALID_ARGUMENT = 2,
    GM_ERR_REQUEST = 3,
    GM_ERR_RESPONSE_PARSE = 4,
    GM_ERR_TOKEN_PARSE = 5,
    GM_ERR_JSON = 6,
    GM_ERR_INTERNAL = 7,
}

#[repr(C)]
#[derive(Debug)]
pub struct gm_string_t {
    pub ptr: *mut c_char,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug)]
pub struct gm_message_t {
    pub mail_id: gm_string_t,
    pub mail_from: gm_string_t,
    pub mail_subject: gm_string_t,
    pub mail_excerpt: gm_string_t,
    pub mail_timestamp: gm_string_t,
}

#[repr(C)]
#[derive(Debug)]
pub struct gm_message_list_t {
    pub ptr: *mut gm_message_t,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug)]
pub struct gm_email_details_t {
    pub mail_id: gm_string_t,
    pub mail_from: gm_string_t,
    pub mail_subject: gm_string_t,
    pub mail_body: gm_string_t,
    pub mail_timestamp: gm_string_t,
    pub attachment_count: u32,
    pub has_attachment_count: bool,
}

impl Default for gm_string_t {
    fn default() -> Self {
        Self {
            ptr: ptr::null_mut(),
            len: 0,
        }
    }
}

impl Default for gm_message_list_t {
    fn default() -> Self {
        Self {
            ptr: ptr::null_mut(),
            len: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct BuilderState {
    proxy: Option<String>,
    danger_accept_invalid_certs: bool,
    user_agent: Option<String>,
    timeout_ms: u64,
    #[cfg(test)]
    ajax_url: Option<String>,
    #[cfg(test)]
    base_url: Option<String>,
}

impl Default for BuilderState {
    fn default() -> Self {
        Self {
            proxy: None,
            danger_accept_invalid_certs: true,
            user_agent: None,
            timeout_ms: 30_000,
            #[cfg(test)]
            ajax_url: None,
            #[cfg(test)]
            base_url: None,
        }
    }
}

impl BuilderState {
    fn to_client_builder(&self) -> ClientBuilder {
        let mut builder = Client::builder()
            .danger_accept_invalid_certs(self.danger_accept_invalid_certs)
            .timeout(Duration::from_millis(self.timeout_ms));

        if let Some(proxy) = &self.proxy {
            builder = builder.proxy(proxy.clone());
        }
        if let Some(user_agent) = &self.user_agent {
            builder = builder.user_agent(user_agent.clone());
        }
        #[cfg(test)]
        {
            if let Some(ajax_url) = &self.ajax_url {
                builder = builder.ajax_url(ajax_url.clone());
            }
            if let Some(base_url) = &self.base_url {
                builder = builder.base_url(base_url.clone());
            }
        }
        builder
    }

    #[cfg(test)]
    fn with_test_urls(mut self, base_url: String, ajax_url: String) -> Self {
        self.base_url = Some(base_url);
        self.ajax_url = Some(ajax_url);
        self
    }
}

struct ClientHandle {
    runtime: Runtime,
    client: Client,
}

fn with_ffi_boundary<F>(f: F) -> gm_status_t
where
    F: FnOnce() -> Result<(), gm_status_t>,
{
    clear_last_error();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => gm_status_t::GM_OK,
        Ok(Err(status)) => status,
        Err(_) => {
            set_last_error("panic while executing FFI function");
            gm_status_t::GM_ERR_INTERNAL
        }
    }
}

fn set_last_error(message: impl Into<String>) {
    let mut bytes = message.into().into_bytes();
    bytes.push(0);
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(bytes);
    });
}

fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

fn status_from_error(error: Error) -> gm_status_t {
    let status = match error {
        Error::Request(_) => gm_status_t::GM_ERR_REQUEST,
        Error::ResponseParse(_) => gm_status_t::GM_ERR_RESPONSE_PARSE,
        Error::TokenParse => gm_status_t::GM_ERR_TOKEN_PARSE,
        Error::Json(_) => gm_status_t::GM_ERR_JSON,
        Error::Regex(_) | Error::HeaderValue(_) | Error::DomainParse => {
            gm_status_t::GM_ERR_INTERNAL
        }
    };
    set_last_error(error.to_string());
    status
}

fn runtime_new() -> Result<Runtime, gm_status_t> {
    RuntimeBuilder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            set_last_error(format!("failed to create Tokio runtime: {error}"));
            gm_status_t::GM_ERR_INTERNAL
        })
}

fn null_error(message: &str) -> gm_status_t {
    set_last_error(message);
    gm_status_t::GM_ERR_NULL
}

fn invalid_arg(message: &str) -> gm_status_t {
    set_last_error(message);
    gm_status_t::GM_ERR_INVALID_ARGUMENT
}

unsafe fn builder_state_mut<'a>(
    builder: *mut gm_builder_t,
) -> Result<&'a mut BuilderState, gm_status_t> {
    if builder.is_null() {
        return Err(null_error("builder is null"));
    }
    Ok(unsafe { &mut *builder.cast::<BuilderState>() })
}

unsafe fn client_handle_ref<'a>(client: *mut gm_client_t) -> Result<&'a ClientHandle, gm_status_t> {
    if client.is_null() {
        return Err(null_error("client is null"));
    }
    Ok(unsafe { &*client.cast::<ClientHandle>() })
}

unsafe fn read_required_str(ptr: *const c_char, name: &str) -> Result<String, gm_status_t> {
    if ptr.is_null() {
        return Err(null_error(&format!("{name} is null")));
    }
    let value = unsafe { CStr::from_ptr(ptr) };
    value
        .to_str()
        .map(str::to_owned)
        .map_err(|_| invalid_arg(&format!("{name} is not valid UTF-8")))
}

fn into_gm_string(value: String) -> gm_string_t {
    let mut bytes = value.into_bytes();
    let len = bytes.len();
    bytes.push(0);
    let ptr = bytes.as_mut_ptr().cast::<c_char>();
    mem::forget(bytes);
    gm_string_t { ptr, len }
}

fn free_gm_string(value: &mut gm_string_t) {
    if value.ptr.is_null() {
        value.len = 0;
        return;
    }
    let len_with_nul = value.len.saturating_add(1);
    unsafe {
        let _ = Vec::from_raw_parts(value.ptr.cast::<u8>(), len_with_nul, len_with_nul);
    }
    value.ptr = ptr::null_mut();
    value.len = 0;
}

fn into_gm_message(message: Message) -> gm_message_t {
    gm_message_t {
        mail_id: into_gm_string(message.mail_id),
        mail_from: into_gm_string(message.mail_from),
        mail_subject: into_gm_string(message.mail_subject),
        mail_excerpt: into_gm_string(message.mail_excerpt),
        mail_timestamp: into_gm_string(message.mail_timestamp),
    }
}

fn free_gm_message(message: &mut gm_message_t) {
    free_gm_string(&mut message.mail_id);
    free_gm_string(&mut message.mail_from);
    free_gm_string(&mut message.mail_subject);
    free_gm_string(&mut message.mail_excerpt);
    free_gm_string(&mut message.mail_timestamp);
}

fn into_gm_email_details(details: EmailDetails) -> gm_email_details_t {
    gm_email_details_t {
        mail_id: into_gm_string(details.mail_id),
        mail_from: into_gm_string(details.mail_from),
        mail_subject: into_gm_string(details.mail_subject),
        mail_body: into_gm_string(details.mail_body),
        mail_timestamp: into_gm_string(details.mail_timestamp),
        attachment_count: details.attachment_count.unwrap_or(0),
        has_attachment_count: details.attachment_count.is_some(),
    }
}

fn free_gm_email_details_fields(details: &mut gm_email_details_t) {
    free_gm_string(&mut details.mail_id);
    free_gm_string(&mut details.mail_from);
    free_gm_string(&mut details.mail_subject);
    free_gm_string(&mut details.mail_body);
    free_gm_string(&mut details.mail_timestamp);
    details.attachment_count = 0;
    details.has_attachment_count = false;
}

fn build_client_handle(state: &BuilderState) -> Result<*mut gm_client_t, gm_status_t> {
    let runtime = runtime_new()?;
    let client = runtime
        .block_on(state.to_client_builder().build())
        .map_err(status_from_error)?;
    let handle = ClientHandle { runtime, client };
    Ok(Box::into_raw(Box::new(handle)).cast::<gm_client_t>())
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_builder_new(out_builder: *mut *mut gm_builder_t) -> gm_status_t {
    with_ffi_boundary(|| {
        if out_builder.is_null() {
            return Err(null_error("out_builder is null"));
        }
        let builder = Box::new(BuilderState::default());
        unsafe {
            *out_builder = Box::into_raw(builder).cast::<gm_builder_t>();
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_builder_free(builder: *mut gm_builder_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if builder.is_null() {
            return;
        }
        unsafe {
            let _ = Box::from_raw(builder.cast::<BuilderState>());
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_builder_set_proxy(
    builder: *mut gm_builder_t,
    proxy: *const c_char,
) -> gm_status_t {
    with_ffi_boundary(|| {
        let state = unsafe { builder_state_mut(builder)? };
        if proxy.is_null() {
            state.proxy = None;
            return Ok(());
        }
        state.proxy = Some(unsafe { read_required_str(proxy, "proxy")? });
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_builder_set_danger_accept_invalid_certs(
    builder: *mut gm_builder_t,
    value: bool,
) -> gm_status_t {
    with_ffi_boundary(|| {
        let state = unsafe { builder_state_mut(builder)? };
        state.danger_accept_invalid_certs = value;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_builder_set_user_agent(
    builder: *mut gm_builder_t,
    user_agent: *const c_char,
) -> gm_status_t {
    with_ffi_boundary(|| {
        let state = unsafe { builder_state_mut(builder)? };
        state.user_agent = Some(unsafe { read_required_str(user_agent, "user_agent")? });
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_builder_set_timeout_ms(
    builder: *mut gm_builder_t,
    timeout_ms: u64,
) -> gm_status_t {
    with_ffi_boundary(|| {
        let state = unsafe { builder_state_mut(builder)? };
        if timeout_ms == 0 {
            return Err(invalid_arg("timeout_ms must be greater than zero"));
        }
        state.timeout_ms = timeout_ms;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_builder_build(
    builder: *mut gm_builder_t,
    out_client: *mut *mut gm_client_t,
) -> gm_status_t {
    with_ffi_boundary(|| {
        if out_client.is_null() {
            return Err(null_error("out_client is null"));
        }
        let state = unsafe { builder_state_mut(builder)? };
        let client = build_client_handle(state)?;
        unsafe {
            *out_client = client;
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_client_new_default(out_client: *mut *mut gm_client_t) -> gm_status_t {
    with_ffi_boundary(|| {
        if out_client.is_null() {
            return Err(null_error("out_client is null"));
        }
        let state = BuilderState::default();
        let client = build_client_handle(&state)?;
        unsafe {
            *out_client = client;
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_client_free(client: *mut gm_client_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if client.is_null() {
            return;
        }
        unsafe {
            let _ = Box::from_raw(client.cast::<ClientHandle>());
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_client_create_email(
    client: *mut gm_client_t,
    alias: *const c_char,
    out_email: *mut gm_string_t,
) -> gm_status_t {
    with_ffi_boundary(|| {
        if out_email.is_null() {
            return Err(null_error("out_email is null"));
        }
        let handle = unsafe { client_handle_ref(client)? };
        let alias = unsafe { read_required_str(alias, "alias")? };
        let email = handle
            .runtime
            .block_on(handle.client.create_email(&alias))
            .map_err(status_from_error)?;
        unsafe {
            *out_email = into_gm_string(email);
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_client_get_messages(
    client: *mut gm_client_t,
    email: *const c_char,
    out_messages: *mut gm_message_list_t,
) -> gm_status_t {
    with_ffi_boundary(|| {
        if out_messages.is_null() {
            return Err(null_error("out_messages is null"));
        }
        let handle = unsafe { client_handle_ref(client)? };
        let email = unsafe { read_required_str(email, "email")? };
        let messages = handle
            .runtime
            .block_on(handle.client.get_messages(&email))
            .map_err(status_from_error)?;
        let boxed = messages
            .into_iter()
            .map(into_gm_message)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let len = boxed.len();
        let ptr = Box::into_raw(boxed).cast::<gm_message_t>();
        unsafe {
            (*out_messages).ptr = ptr;
            (*out_messages).len = len;
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_client_fetch_email(
    client: *mut gm_client_t,
    email: *const c_char,
    mail_id: *const c_char,
    out_details: *mut *mut gm_email_details_t,
) -> gm_status_t {
    with_ffi_boundary(|| {
        if out_details.is_null() {
            return Err(null_error("out_details is null"));
        }
        let handle = unsafe { client_handle_ref(client)? };
        let email = unsafe { read_required_str(email, "email")? };
        let mail_id = unsafe { read_required_str(mail_id, "mail_id")? };
        let details = handle
            .runtime
            .block_on(handle.client.fetch_email(&email, &mail_id))
            .map_err(status_from_error)?;
        let details = Box::new(into_gm_email_details(details));
        unsafe {
            *out_details = Box::into_raw(details);
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_client_delete_email(
    client: *mut gm_client_t,
    email: *const c_char,
    out_deleted: *mut bool,
) -> gm_status_t {
    with_ffi_boundary(|| {
        if out_deleted.is_null() {
            return Err(null_error("out_deleted is null"));
        }
        let handle = unsafe { client_handle_ref(client)? };
        let email = unsafe { read_required_str(email, "email")? };
        let deleted = handle
            .runtime
            .block_on(handle.client.delete_email(&email))
            .map_err(status_from_error)?;
        unsafe {
            *out_deleted = deleted;
        }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_string_free(value: *mut gm_string_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if value.is_null() {
            return;
        }
        let value = unsafe { &mut *value };
        free_gm_string(value);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_message_list_free(messages: *mut gm_message_list_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if messages.is_null() {
            return;
        }
        let messages = unsafe { &mut *messages };
        if !messages.ptr.is_null() {
            let slice_ptr = ptr::slice_from_raw_parts_mut(messages.ptr, messages.len);
            let mut boxed = unsafe { Box::from_raw(slice_ptr) };
            for message in boxed.iter_mut() {
                free_gm_message(message);
            }
        }
        messages.ptr = ptr::null_mut();
        messages.len = 0;
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_email_details_free(details: *mut gm_email_details_t) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if details.is_null() {
            return;
        }
        let mut details = unsafe { Box::from_raw(details) };
        free_gm_email_details_fields(&mut details);
    }));
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_last_error_message() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|bytes| bytes.as_ptr().cast::<c_char>())
            .unwrap_or(ptr::null())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn gm_last_error_clear() {
    clear_last_error();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    struct TestServer {
        addr: SocketAddr,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
            listener.set_nonblocking(false).expect("set blocking mode");
            let addr = listener.local_addr().expect("local addr");
            let stop = Arc::new(AtomicBool::new(false));
            let stop_flag = Arc::clone(&stop);

            let thread = thread::spawn(move || {
                while !stop_flag.load(Ordering::Relaxed) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(pair) => pair,
                        Err(_) => continue,
                    };
                    let mut buffer = [0_u8; 8192];
                    let bytes_read = match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => continue,
                        Ok(n) => n,
                    };
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let mut lines = request.lines();
                    let request_line = lines.next().unwrap_or_default();

                    let response = if request_line.starts_with("GET / ") {
                        http_response(
                            "200 OK",
                            "text/html",
                            "window.GM = { api_token : 'test-token' };",
                        )
                    } else if request_line.starts_with("POST /ajax.php?f=set_email_user ") {
                        if request.contains("X-Requested-With: XMLHttpRequest")
                            && request.contains("Authorization: ApiToken test-token")
                        {
                            http_response(
                                "200 OK",
                                "application/json",
                                r#"{"email_addr":"demoalias@sharklasers.com"}"#,
                            )
                        } else {
                            http_response("400 Bad Request", "text/plain", "missing headers")
                        }
                    } else if request_line.starts_with("GET /ajax.php?email_id=1&f=fetch_email ")
                        || request_line.starts_with("GET /ajax.php?f=fetch_email&email_id=1 ")
                    {
                        http_response(
                            "200 OK",
                            "application/json",
                            r#"{"mail_id":"1","mail_from":"sender@example.com","mail_subject":"Subject","mail_body":"<p>Body</p>","mail_timestamp":"1700000000","att":"1"}"#,
                        )
                    } else if request_line.starts_with("GET /ajax.php?seq=1&f=check_email ")
                        || request_line.starts_with("GET /ajax.php?f=check_email&seq=1 ")
                    {
                        http_response(
                            "200 OK",
                            "application/json",
                            r#"{"list":[{"mail_id":"1","mail_from":"sender@example.com","mail_subject":"Subject","mail_excerpt":"Excerpt","mail_timestamp":"1700000000"}]}"#,
                        )
                    } else if request_line.starts_with("POST /ajax.php?f=forget_me ") {
                        http_response("200 OK", "application/json", r#"{"success":true}"#)
                    } else {
                        http_response("404 Not Found", "text/plain", "not found")
                    };

                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                }
            });

            Self {
                addr,
                stop,
                thread: Some(thread),
            }
        }

        fn base_url(&self) -> String {
            format!("http://{}", self.addr)
        }

        fn ajax_url(&self) -> String {
            format!("{}/ajax.php", self.base_url())
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = std::net::TcpStream::connect(self.addr);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn http_response(status: &str, content_type: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn test_builder_handle(server: &TestServer) -> *mut gm_builder_t {
        let state = BuilderState::default().with_test_urls(server.base_url(), server.ajax_url());
        Box::into_raw(Box::new(state)).cast::<gm_builder_t>()
    }

    fn c_string_ptr(value: &str) -> Vec<u8> {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        bytes
    }

    #[test]
    fn gm_string_free_is_null_safe() {
        gm_string_free(ptr::null_mut());

        let mut value = gm_string_t::default();
        gm_string_free(&mut value);
        assert!(value.ptr.is_null());
        assert_eq!(value.len, 0);
    }

    #[test]
    fn gm_builder_set_timeout_rejects_zero() {
        let mut builder = ptr::null_mut();
        assert_eq!(gm_builder_new(&mut builder), gm_status_t::GM_OK);

        let status = gm_builder_set_timeout_ms(builder, 0);
        assert_eq!(status, gm_status_t::GM_ERR_INVALID_ARGUMENT);
        let error = unsafe { CStr::from_ptr(gm_last_error_message()) }
            .to_str()
            .expect("utf8 error");
        assert!(error.contains("timeout_ms"));

        gm_builder_free(builder);
    }

    #[test]
    fn ffi_flow_uses_mock_server() {
        let server = TestServer::start();
        let builder = test_builder_handle(&server);
        let ua = c_string_ptr("ffi-test/1.0");
        assert_eq!(
            gm_builder_set_user_agent(builder, ua.as_ptr().cast::<c_char>()),
            gm_status_t::GM_OK
        );

        let mut client = ptr::null_mut();
        assert_eq!(gm_builder_build(builder, &mut client), gm_status_t::GM_OK);
        gm_builder_free(builder);

        let alias = c_string_ptr("demoalias");
        let mut email = gm_string_t::default();
        assert_eq!(
            gm_client_create_email(client, alias.as_ptr().cast::<c_char>(), &mut email),
            gm_status_t::GM_OK
        );
        let email_str = unsafe { CStr::from_ptr(email.ptr) }
            .to_str()
            .expect("email utf8");
        assert_eq!(email_str, "demoalias@sharklasers.com");

        let mut messages = gm_message_list_t::default();
        assert_eq!(
            gm_client_get_messages(client, email.ptr.cast::<c_char>(), &mut messages),
            gm_status_t::GM_OK
        );
        assert_eq!(messages.len, 1);
        let first_message = unsafe { &*messages.ptr };
        let mail_id = unsafe { CStr::from_ptr(first_message.mail_id.ptr) }
            .to_str()
            .expect("mail_id utf8");
        assert_eq!(mail_id, "1");

        let mut details = ptr::null_mut();
        assert_eq!(
            gm_client_fetch_email(
                client,
                email.ptr.cast::<c_char>(),
                first_message.mail_id.ptr.cast::<c_char>(),
                &mut details,
            ),
            gm_status_t::GM_OK
        );
        let details = unsafe { &*details };
        let subject = unsafe { CStr::from_ptr(details.mail_subject.ptr) }
            .to_str()
            .expect("subject utf8");
        assert_eq!(subject, "Subject");
        assert!(details.has_attachment_count);
        assert_eq!(details.attachment_count, 1);

        let mut deleted = false;
        assert_eq!(
            gm_client_delete_email(client, email.ptr.cast::<c_char>(), &mut deleted),
            gm_status_t::GM_OK
        );
        assert!(deleted);

        gm_email_details_free(details as *const gm_email_details_t as *mut gm_email_details_t);
        gm_message_list_free(&mut messages);
        gm_string_free(&mut email);
        gm_client_free(client);
    }

    #[test]
    fn null_client_reports_error() {
        let alias = c_string_ptr("demoalias");
        let mut email = gm_string_t::default();
        let status =
            gm_client_create_email(ptr::null_mut(), alias.as_ptr().cast::<c_char>(), &mut email);
        assert_eq!(status, gm_status_t::GM_ERR_NULL);
        let error = unsafe { CStr::from_ptr(gm_last_error_message()) }
            .to_str()
            .expect("utf8 error");
        assert!(error.contains("client"));
    }
}
