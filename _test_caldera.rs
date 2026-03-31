use usd_core::{InitialLoadSet, Stage};

fn main() {
    let path = r"C:/projects/projects.rust.cg/usd-refs/caldera/assets/xmodel/characters/iw8/parts_boots_work.gdt.usd";
    eprintln!("Opening stage: {}", path);
    let stage = match Stage::open(path.to_string(), InitialLoadSet::LoadAll) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FAILED to open: {:?}", e);
            return;
        }
    };
    eprintln!("Stage opened OK");
    eprintln!("Root layer: {:?}", stage.root_layer().get_identifier());
    eprintln!("Default prim: {:?}", stage.get_default_prim());
    
    // Traverse all prims
    let root = stage.get_pseudo_root();
    let mut count = 0;
    for prim in root.get_descendants() {
        count += 1;
        let path = prim.get_path();
        let type_name = prim.get_type_name();
        let is_loaded = prim.is_loaded();
        let is_active = prim.is_active();
        eprintln!("  Prim: {} type={} loaded={} active={}", path.get_string(), type_name, is_loaded, is_active);
        if count > 50 { 
            eprintln!("  ... (truncated at 50 prims)");
            break; 
        }
    }
    eprintln!("Total prim count: {}", count);
}
