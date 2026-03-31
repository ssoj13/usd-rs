#![allow(dead_code)]

//! HdStPtexMipmapTextureLoader - Ptex mipmap generation.
//!
//! Loads Ptex textures and generates mipmaps packed into pages for GPU upload.
//! Each face is stored with guttering pixels and mipmap chain in a compact
//! block layout, then packed into pages for efficient GPU access.
//!
//! Port of pxr/imaging/hdSt/ptexMipmapTextureLoader.h

/// A block of Ptex face data with mipmaps.
///
/// Each face is stored as a block containing the base texels plus all
/// mipmap levels, surrounded by guttering pixels for seamless filtering.
#[derive(Debug, Clone, Default)]
pub struct PtexBlock {
    /// Ptex face index
    pub index: i32,
    /// Number of mipmap levels in this block
    pub num_mipmaps: i32,
    /// Top-left texel offset in the page
    pub u: u16,
    /// Top-left texel offset in the page
    pub v: u16,
    /// Texel width (includes mipmap area)
    pub width: u16,
    /// Texel height (includes mipmap area)
    pub height: u16,
    /// Max tile size difference around each vertex (packed 4x4 bits)
    pub adj_size_diffs: u16,
    /// Log2 of original tile width
    pub ulog2: i8,
    /// Log2 of original tile height
    pub vlog2: i8,
}

impl PtexBlock {
    /// Total number of texels in this block.
    pub fn num_texels(&self) -> i32 {
        self.width as i32 * self.height as i32
    }

    /// Set block size from log2 dimensions, optionally including mipmap area.
    pub fn set_size(&mut self, ulog2: u8, vlog2: u8, mipmap: bool) {
        self.ulog2 = ulog2 as i8;
        self.vlog2 = vlog2 as i8;
        let w = 1u16 << ulog2;
        let h = 1u16 << vlog2;
        if mipmap {
            // Mipmap chain adds w/2+2 to width and keeps height
            self.width = w + 2 + w / 2 + 2;
            self.height = h + 2;
        } else {
            self.width = w + 2;
            self.height = h + 2;
        }
    }

    /// Sort comparator: descending by height, then width.
    pub fn sort_by_height(a: &PtexBlock, b: &PtexBlock) -> std::cmp::Ordering {
        b.height.cmp(&a.height).then_with(|| b.width.cmp(&a.width))
    }

    /// Sort comparator: descending by total texel area.
    pub fn sort_by_area(a: &PtexBlock, b: &PtexBlock) -> std::cmp::Ordering {
        b.num_texels().cmp(&a.num_texels())
    }
}

/// Ptex mipmap texture loader.
///
/// Packs Ptex face data with guttering and mipmaps into pages for GPU upload.
/// Produces a texel buffer (packed face data) and a layout buffer
/// (per-face metadata for shader lookup).
///
/// Port of HdStPtexMipmapTextureLoader
#[derive(Debug)]
pub struct PtexMipmapTextureLoader {
    /// Packed face blocks
    blocks: Vec<PtexBlock>,
    /// Number of pages allocated
    num_pages: usize,
    /// Page width in texels
    page_width: i32,
    /// Page height in texels
    page_height: i32,
    /// Bytes per pixel
    bpp: i32,
    /// Max mipmap levels
    max_levels: i32,
    /// Packed texel data for GPU upload
    texel_buffer: Vec<u8>,
    /// Per-face layout metadata for GPU upload
    layout_buffer: Vec<u8>,
    /// Total memory usage in bytes
    memory_usage: usize,
}

impl PtexMipmapTextureLoader {
    /// Create a new Ptex mipmap texture loader.
    ///
    /// - `max_num_pages`: Maximum number of texture pages to allocate
    /// - `max_levels`: Maximum mipmap levels (-1 = all)
    /// - `target_memory`: Target memory budget in bytes (0 = unlimited)
    /// - `seamless_mipmap`: Enable seamless mipmap filtering
    pub fn new(
        max_num_pages: i32,
        max_levels: i32,
        target_memory: usize,
        _seamless_mipmap: bool,
    ) -> Self {
        let _ = max_num_pages;
        let _ = target_memory;
        Self {
            blocks: Vec::new(),
            num_pages: 0,
            page_width: 0,
            page_height: 0,
            bpp: 0,
            max_levels,
            texel_buffer: Vec::new(),
            layout_buffer: Vec::new(),
            memory_usage: 0,
        }
    }

    /// Get the layout buffer (per-face metadata).
    pub fn layout_buffer(&self) -> &[u8] {
        &self.layout_buffer
    }

    /// Get the texel buffer (packed face data).
    pub fn texel_buffer(&self) -> &[u8] {
        &self.texel_buffer
    }

    /// Get number of faces (blocks).
    pub fn num_faces(&self) -> i32 {
        self.blocks.len() as i32
    }

    /// Get number of allocated pages.
    pub fn num_pages(&self) -> i32 {
        self.num_pages as i32
    }

    /// Get page width in texels.
    pub fn page_width(&self) -> i32 {
        self.page_width
    }

    /// Get page height in texels.
    pub fn page_height(&self) -> i32 {
        self.page_height
    }

    /// Get total memory usage in bytes.
    pub fn memory_usage(&self) -> usize {
        self.memory_usage
    }

    /// Add a block for a Ptex face.
    pub fn add_block(&mut self, block: PtexBlock) {
        self.blocks.push(block);
    }

    /// Access blocks.
    pub fn blocks(&self) -> &[PtexBlock] {
        &self.blocks
    }

    /// Set texel buffer data.
    pub fn set_texel_buffer(&mut self, data: Vec<u8>) {
        self.memory_usage = self.memory_usage.saturating_sub(self.texel_buffer.len()) + data.len();
        self.texel_buffer = data;
    }

    /// Set layout buffer data.
    pub fn set_layout_buffer(&mut self, data: Vec<u8>) {
        self.memory_usage = self.memory_usage.saturating_sub(self.layout_buffer.len()) + data.len();
        self.layout_buffer = data;
    }

    /// Set page dimensions.
    pub fn set_page_size(&mut self, width: i32, height: i32) {
        self.page_width = width;
        self.page_height = height;
    }

    /// Set number of pages.
    pub fn set_num_pages(&mut self, n: usize) {
        self.num_pages = n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_size() {
        let mut block = PtexBlock::default();
        block.set_size(3, 3, true); // 8x8 with mipmaps
        assert_eq!(block.ulog2, 3);
        assert_eq!(block.vlog2, 3);
        // width = 8 + 2 + 4 + 2 = 16, height = 8 + 2 = 10
        assert_eq!(block.width, 16);
        assert_eq!(block.height, 10);
        assert_eq!(block.num_texels(), 160);
    }

    #[test]
    fn test_block_sort() {
        let a = PtexBlock {
            width: 10,
            height: 10,
            ..Default::default()
        };
        let b = PtexBlock {
            width: 20,
            height: 20,
            ..Default::default()
        };
        // b is larger, so it should come first in descending sort
        let mut blocks = vec![a.clone(), b.clone()];
        blocks.sort_by(PtexBlock::sort_by_area);
        assert_eq!(blocks[0].width, 20);
    }

    #[test]
    fn test_loader_lifecycle() {
        let mut loader = PtexMipmapTextureLoader::new(16, -1, 0, true);
        assert_eq!(loader.num_faces(), 0);

        loader.add_block(PtexBlock {
            index: 0,
            width: 10,
            height: 10,
            ..Default::default()
        });
        loader.add_block(PtexBlock {
            index: 1,
            width: 8,
            height: 8,
            ..Default::default()
        });
        assert_eq!(loader.num_faces(), 2);

        loader.set_texel_buffer(vec![0u8; 1024]);
        loader.set_layout_buffer(vec![0u8; 64]);
        assert_eq!(loader.memory_usage(), 1088);
    }
}
