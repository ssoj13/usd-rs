//! File writer interface.
//! Reference: `_ref/draco/src/draco/io/file_writer_interface.h`.

pub trait FileWriterInterface {
    fn write(&mut self, buffer: &[u8]) -> bool;
}
