fn main() {
    if cfg!(target_endian = "little") {
        println!("Little-endian")
    }
    if cfg!(target_endian = "big") {
        println!("Big-endian")
    }
    if cfg!(target_pointer_width = "32") {
        println!("32-bit")
    }
    if cfg!(target_pointer_width = "64") {
        println!("64-bit")
    }
}
