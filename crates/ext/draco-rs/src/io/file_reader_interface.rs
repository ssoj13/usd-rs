//! File reader interface.
//! Reference: `_ref/draco/src/draco/io/file_reader_interface.h`.

pub trait FileReaderInterface {
    fn read_file_to_buffer(&mut self, buffer: &mut Vec<u8>) -> bool;
    fn read_file_to_buffer_u8(&mut self, buffer: &mut Vec<u8>) -> bool {
        self.read_file_to_buffer(buffer)
    }
    fn get_file_size(&mut self) -> usize;
}
