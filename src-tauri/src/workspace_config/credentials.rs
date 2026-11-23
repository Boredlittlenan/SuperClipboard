#[cfg(windows)]
use base64::{engine::general_purpose::STANDARD, Engine};

#[cfg(windows)]
pub fn encode(url: &str) -> Result<String, String> {
    crypt(url.as_bytes(), true).map(|value| STANDARD.encode(value))
}

#[cfg(windows)]
pub fn decode(value: &str) -> Result<String, String> {
    let encoded = STANDARD
        .decode(value)
        .map_err(|_| "Invalid saved PostgreSQL credentials")?;
    String::from_utf8(crypt(&encoded, false)?).map_err(|_| "Invalid saved PostgreSQL URI".into())
}

#[cfg(windows)]
fn crypt(value: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let len = value
        .len()
        .try_into()
        .map_err(|_| "PostgreSQL URI is too large")?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: len,
        pbData: value.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // DPAPI allocates the output buffer with LocalAlloc. The input is borrowed for the call.
    unsafe {
        let result = if encrypt {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        result.map_err(|e| format!("Windows credential protection failed: {e}"))?;
        if output.pbData.is_null() {
            return Err("Windows returned empty credentials".into());
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output.pbData.cast()));
        Ok(bytes)
    }
}

#[cfg(not(windows))]
pub fn encode(_url: &str) -> Result<String, String> {
    Err("Saving external storage credentials requires Windows".into())
}

#[cfg(not(windows))]
pub fn decode(_value: &str) -> Result<String, String> {
    Err("Saved credentials belong to a Windows account".into())
}
