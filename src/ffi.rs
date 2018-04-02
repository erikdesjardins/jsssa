use std::slice;

/// Lossily convert a non-null pointer to a null-terminated UCS-2 (or UTF-16) string into a Rust string.
/// The pointer must point to a null-terminated string in valid memory.
/// O(n) runtime.
pub unsafe fn ucs2_to_string(ptr: *const u16) -> String {
    let mut count = 0;
    let mut p = ptr;
    while *p != 0 {
        count += 1;
        p = p.offset(1);
    }
    String::from_utf16_lossy(slice::from_raw_parts(ptr, count))
}
