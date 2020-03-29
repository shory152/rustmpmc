//!
//! inspect memory with these functions
//!

///
/// dump memory from `addr`, with `len` byte
///
/// # Examples
///
/// ```
/// #[ignore]
/// use rust1::dump::*;
/// let a = 123;
/// dumpBytes(&a as *const i32 as usize, 4);
/// ```
///
pub fn dumpBytes(addr : usize, len : usize) {
    unsafe {
        for i in 0..len {
            if i % 8 == 0 {
                print!("0x{:016x}: ", addr+i);
            }
            print!("{:02x} ", *((addr+i) as *const u8));

            if (i+1) % 8 == 0 || (i+1) == len{
                println!();
            }
        }
    }
}

pub fn dump<T>(label : &str, addr : &T, size : i32) {
    let start = addr as * const T as usize;
    let tlen = std::mem::size_of_val(addr);
    println!("dump {} {:x} ~ {:x} ({})", label, start, start+ size as usize, tlen);

    dumpBytes(start, size as usize);
}

pub fn dumpRef<T>(label : &str, addr : &T) {
    let start = addr as * const T as usize;
    let tlen = std::mem::size_of_val(addr);
    println!("dump {} {:x} ~ {:x} ({})", label, start, start+ tlen as usize, tlen);

    dumpBytes(start, tlen as usize);
}


pub fn dumpDref<T>(label : &str, addr : &T, size : i32) {
    let start = addr as * const T as usize;
    let tlen = std::mem::size_of_val(addr);
    let usize_len = std::mem::size_of::<usize>();
    println!("dump dref {} :", label);
    dumpBytes(start, 8);
    unsafe {
        let ptr = *(start as *const *const u8);
        println!("dump dref *{} : {:x}~{:x}", label, ptr as usize, ptr as usize + size as usize);
        dumpBytes(ptr as usize, size as usize);
    }
}