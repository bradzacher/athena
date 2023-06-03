use clean_path::Clean;
use ignore::{types::TypesBuilder, WalkBuilder, WalkState};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub fn get_files(paths: &Vec<PathBuf>) -> Vec<PathBuf> {
    let mut types_builder = TypesBuilder::new();
    types_builder
        .add("typescript", "*.{cts,mts,ts,tsx}")
        .expect("Invalid glob");
    types_builder.select("typescript");
    types_builder
        .add("javascript", "*.{cjs,mjs,js,jsx}")
        .expect("Invalid glob");
    types_builder.select("javascript");
    let types = types_builder.build().expect("Unable to build types");

    let mut walk_builder = WalkBuilder::new(paths[0].to_owned());
    if paths.len() > 1 {
        for path in paths.iter().skip(1) {
            walk_builder.add(path.to_owned());
        }
    }
    walk_builder.types(types);

    let files = Arc::new(Mutex::new(vec![]));

    /*
    NOTE: we could implement this using a custom `.visit` implementation that defers any shared memory operations until
    the end of each thread to avoid any time spent waiting for the mutex.
    However in practice the cost of the mutex on each write doesn't add any overhead worth mentioning - so instead we
    prefer the much simpler code.

    Tested with `hyperfine --warmup 2 './target/release/athena ../../work/canva/web/src`:

    Single threaded:
      Time (mean ± σ):      1.473 s ±  0.020 s    [User: 0.491 s, System: 0.975 s]
      Range (min … max):    1.446 s …  1.506 s    10 runs

    Parallel mutex-heavy:
      Time (mean ± σ):     835.0 ms ±  13.9 ms    [User: 539.7 ms, System: 1093.2 ms]
      Range (min … max):   815.3 ms … 859.4 ms    10 runs

    Parallel mutex-light:
      Time (mean ± σ):     839.6 ms ±  10.9 ms    [User: 540.0 ms, System: 1104.4 ms]
      Range (min … max):   819.9 ms … 851.4 ms    10 runs
    */
    walk_builder.build_parallel().run(|| {
        let files = files.clone();
        return Box::new(move |result| {
            // Each item yielded by the iterator is either a directory entry or an
            // error, so either handle the path or the error.
            match result {
                Ok(entry) => match entry.file_type() {
                    Some(file_type) => {
                        if !file_type.is_dir() {
                            files.lock().unwrap().push(entry.path().to_owned().clean());
                        }
                    }
                    None => {
                        // ignore non-file entries
                    }
                },
                Err(err) => println!("ERROR: {}", err),
            };
            return WalkState::Continue;
        });
    });

    return files.lock().unwrap().to_vec();
}

pub fn is_declaration_file<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    return path.ends_with(".d.ts") || path.ends_with(".d.mts") || path.ends_with(".d.cts");
}
