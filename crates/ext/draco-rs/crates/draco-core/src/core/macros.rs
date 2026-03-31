//! Macro utilities.
//! Reference: `_ref/draco/src/draco/core/macros.h`.

pub const DRACO_DEBUG: bool = cfg!(debug_assertions);

#[macro_export]
macro_rules! draco_dcheck {
    ($x:expr) => {{
        if $crate::core::macros::DRACO_DEBUG {
            assert!($x);
        }
    }};
}

#[macro_export]
macro_rules! draco_dcheck_eq {
    ($a:expr, $b:expr) => {{
        if $crate::core::macros::DRACO_DEBUG {
            assert_eq!($a, $b);
        }
    }};
}

#[macro_export]
macro_rules! draco_dcheck_ne {
    ($a:expr, $b:expr) => {{
        if $crate::core::macros::DRACO_DEBUG {
            assert_ne!($a, $b);
        }
    }};
}

#[macro_export]
macro_rules! draco_dcheck_ge {
    ($a:expr, $b:expr) => {{
        if $crate::core::macros::DRACO_DEBUG {
            assert!($a >= $b);
        }
    }};
}

#[macro_export]
macro_rules! draco_dcheck_gt {
    ($a:expr, $b:expr) => {{
        if $crate::core::macros::DRACO_DEBUG {
            assert!($a > $b);
        }
    }};
}

#[macro_export]
macro_rules! draco_dcheck_le {
    ($a:expr, $b:expr) => {{
        if $crate::core::macros::DRACO_DEBUG {
            assert!($a <= $b);
        }
    }};
}

#[macro_export]
macro_rules! draco_dcheck_lt {
    ($a:expr, $b:expr) => {{
        if $crate::core::macros::DRACO_DEBUG {
            assert!($a < $b);
        }
    }};
}

#[macro_export]
macro_rules! draco_dcheck_notnull {
    ($x:expr) => {{
        if $crate::core::macros::DRACO_DEBUG {
            assert!(!$x.is_null());
        }
    }};
}

#[macro_export]
macro_rules! bitstream_version {
    ($major:expr, $minor:expr) => {{
        (($major as u16) << 8) | ($minor as u16)
    }};
}

#[macro_export]
macro_rules! bitstream_version_major {
    ($version:expr) => {{
        (($version >> 8) & 0xFF) as u8
    }};
}

#[macro_export]
macro_rules! bitstream_version_minor {
    ($version:expr) => {{
        ($version & 0xFF) as u8
    }};
}
