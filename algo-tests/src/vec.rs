fn f1() {
    let v1: Vec<_> = (0..100).collect();
    let v2: Vec<_> = (0..100).collect();
    println!("{v1:?} {v2:?}");
    println!("finish");
}

fn f2() {
    let v1: Vec<_> = (0..100).collect();
    println!("{v1:?}");
    drop(v1);
    let v2: Vec<_> = (0..100).collect();
    println!("{v2:?}");
    drop(v2);
}

fn f3() {
    let v1: Vec<_> = (0..100).collect();
    let mut r = &v1;
    if 0 < v1.len() {
        let v2: Vec<_> = (0..100).collect();
        r = &v2;
    }
    println!("{r:?}");
}
