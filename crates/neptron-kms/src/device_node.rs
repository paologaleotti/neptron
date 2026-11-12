//! What a launcher descriptor must be before the backend takes it.

/// The major number of DRM device nodes.
pub const DRM_MAJOR: u32 = 226;
/// The highest standard descriptor: 0 is standard input, 1 standard
/// output, 2 standard error. The launcher never passes one of them.
pub const STANDARD_FD_MAX: i32 = 2;

/// The major number of a Linux device number, as glibc's `major()` takes
/// it apart.
pub fn major(rdev: u64) -> u32 {
    let high = (rdev >> 32) & 0xffff_f000;
    let low = (rdev >> 8) & 0x0fff;
    // Both parts together hold at most 32 bits.
    u32::try_from(high | low).unwrap_or(u32::MAX)
}

/// True for a character device with the DRM major number.
pub fn is_drm_node(is_char_device: bool, rdev: u64) -> bool {
    is_char_device && major(rdev) == DRM_MAJOR
}

#[cfg(test)]
mod tests {
    use super::*;

    /// glibc's `makedev`.
    fn makedev(major: u64, minor: u64) -> u64 {
        ((major & 0xffff_f000) << 32)
            | ((major & 0x0fff) << 8)
            | ((minor & 0xffff_ff00) << 12)
            | (minor & 0xff)
    }

    #[test]
    fn the_major_comes_out_of_the_device_number() {
        assert_eq!(major(makedev(226, 0)), 226);
        assert_eq!(major(makedev(226, 128)), 226);
        assert_eq!(major(makedev(226, 0x1_2345)), 226);
        assert_eq!(major(makedev(0x12_3456, 7)), 0x12_3456);
        assert_eq!(major(makedev(1, 3)), 1);
    }

    #[test]
    fn only_a_character_device_of_the_drm_major_is_a_drm_node() {
        assert!(is_drm_node(true, makedev(226, 0)));
        assert!(!is_drm_node(false, makedev(226, 0)), "a block device");
        assert!(!is_drm_node(true, makedev(1, 3)), "/dev/null");
        assert!(!is_drm_node(false, 0), "a pipe or a file");
    }
}
