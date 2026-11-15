// Each arm moves a different variable, so that the join point after the
// match receives new states many times; every variable may be moved after the
// match (compiles).
fn many_arms(x: u8) {
    let s0 = String::new();
    let s1 = String::new();
    let s2 = String::new();
    let s3 = String::new();
    let s4 = String::new();
    let s5 = String::new();
    let s6 = String::new();
    let s7 = String::new();
    let s8 = String::new();
    let s9 = String::new();
    let s10 = String::new();
    let s11 = String::new();
    match x {
        0 => drop(s0),
        1 => drop(s1),
        2 => drop(s2),
        3 => drop(s3),
        4 => drop(s4),
        5 => drop(s5),
        6 => drop(s6),
        7 => drop(s7),
        8 => drop(s8),
        9 => drop(s9),
        10 => drop(s10),
        11 => drop(s11),
        _ => {}
    }
    println!("after");
}
