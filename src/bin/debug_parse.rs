//! Debug utility to trace parsing issues

fn main() {
    // Initialize file formats
    usd::sdf::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: debug_parse <file.usda>");
        std::process::exit(1);
    }

    let file_path = &args[1];
    println!("=== Debug parsing: {} ===\n", file_path);

    // Step 1: Read raw file
    let content = std::fs::read_to_string(file_path).expect("Failed to read file");
    println!("File size: {} bytes", content.len());
    println!("File lines: {}", content.lines().count());
    println!();

    // Step 2: Open as layer
    println!("Opening layer...");
    let layer = match usd::sdf::Layer::find_or_open(file_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ERROR opening layer: {:?}", e);
            std::process::exit(1);
        }
    };
    println!("Layer opened successfully");
    println!("Layer identifier: {}", layer.identifier());
    println!();

    // Step 3: Check pseudo root
    let pseudo_root = layer.get_pseudo_root();
    println!("Pseudo root path: {}", pseudo_root.path());
    println!("Pseudo root is dormant: {}", pseudo_root.is_dormant());
    println!();

    // Step 4: Check children
    let children = pseudo_root.name_children();
    println!("Root children count: {}", children.len());
    for (i, child) in children.iter().enumerate() {
        println!(
            "  Child {}: {} (type: {})",
            i,
            child.path(),
            child.type_name()
        );

        // Check grandchildren
        let grandchildren = child.name_children();
        println!("    Grandchildren: {}", grandchildren.len());
        if grandchildren.len() > 0 && grandchildren.len() <= 5 {
            for gc in &grandchildren {
                println!("      - {}", gc.path());
            }
        }
    }
    println!();

    // Step 5: Check layer data directly
    println!("Checking layer data...");
    let root_path = usd::sdf::Path::absolute_root();

    // Check primChildren field
    if let Some(prim_children) = layer.get_field(&root_path, &usd::tf::Token::new("primChildren")) {
        println!("primChildren field exists");
        println!("  Value type: {:?}", prim_children.type_name());
        if let Some(tokens) = prim_children.downcast::<Vec<usd::tf::Token>>() {
            println!(
                "  Children tokens: {:?}",
                tokens.iter().map(|t| t.as_str()).collect::<Vec<_>>()
            );
        } else if let Some(strings) = prim_children.downcast::<Vec<String>>() {
            println!("  Children strings: {:?}", strings);
        } else {
            println!("  Could not extract children list");
        }
    } else {
        println!("primChildren field NOT FOUND on root!");
    }
    println!();

    // Step 6: Count specs by traversing from root
    println!("Counting specs by traversal...");
    let spec_count = std::cell::Cell::new(0usize);
    layer.traverse(&usd::sdf::Path::absolute_root(), &|_path| {
        spec_count.set(spec_count.get() + 1);
    });
    println!("Total specs found: {}", spec_count.get());
    println!();

    // Step 7: Export to string
    println!("Exporting to string...");
    match layer.export_to_string() {
        Ok(text) => {
            println!("Export succeeded");
            println!("Export length: {} chars", text.len());
            println!("Export lines: {}", text.lines().count());
            println!();
            println!("First 500 chars:");
            println!("{}", &text[..text.len().min(500)]);
        }
        Err(e) => {
            eprintln!("ERROR exporting: {:?}", e);
        }
    }
}
