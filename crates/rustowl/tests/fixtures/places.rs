// A field moved out on only one branch (compiles).
fn cond_partial_move(c: bool) {
    let p = (String::new(), String::new());
    if c {
        drop(p.0);
    }
    println!("{}", p.1);
}

// A field moved out is assigned again, so that the whole tuple holds a value
// again (compiles).
fn reinit_field() {
    let mut p = (String::new(), String::new());
    let a = p.0;
    p.0 = a;
    println!("{p:?}");
}

// Assigning a field of a moved tuple does not give the whole tuple a value
// (does not compile).
fn assign_field_after_move() {
    let mut p = (String::new(), String::new());
    let q = p;
    p.0 = String::from("a");
    println!("{} {q:?}", p.0);
}

// A move out of a box through a dereference does not touch the box itself,
// since a place behind a dereference is not a fragment (compiles).
fn move_out_of_box() {
    let b = Box::new(String::new());
    let s = *b;
    println!("{s}");
}
