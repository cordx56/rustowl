// Cases for the range where a borrowed variable must live.
// Functions marked "(does not compile)" are expected to have a red underline
// at the later use of the borrow, and the others are expected to have none.

// Reassignment after the borrowed variable dies in an inner block (compiles).
fn reassign_block() {
    let mut r;
    {
        let a = String::from("a");
        r = &a;
        println!("{r}");
    }
    let b = String::from("b");
    r = &b;
    println!("{r}");
}

// Reassignment after the borrowed variable is moved (compiles).
fn reassign_move() {
    let a = String::from("a");
    let mut r = &a;
    println!("{r}");
    drop(a);
    let b = String::from("b");
    r = &b;
    println!("{r}");
}

// Borrow of a loop-local variable, not used after the loop (compiles).
fn loop_local() {
    let base = String::from("base");
    let mut r = &base;
    for i in 0..3 {
        let s = i.to_string();
        r = &s;
        println!("{r}");
    }
}

// The later use is a formatting macro (does not compile).
fn macro_use(c: bool) {
    let n = Box::new(1);
    let mut r = &n;
    if c {
        let m = Box::new(2);
        r = &m;
    }
    println!("{r}");
}

// The later use is a plain dereference (does not compile).
fn deref_use(c: bool) -> i32 {
    let n = Box::new(1);
    let mut r = &n;
    if c {
        let m = Box::new(2);
        r = &m;
    }
    **r + 1
}

// The later use passes the reference to a function (does not compile).
fn call_use(c: bool) {
    let n = String::from("n");
    let mut r = &n;
    if c {
        let m = String::from("m");
        r = &m;
    }
    takes(r);
}

fn takes(s: &str) {
    println!("{s}");
}

// The later use is through a copied reference (does not compile).
fn copy_use(c: bool) {
    let n = String::from("n");
    let mut r = &n;
    if c {
        let m = String::from("m");
        r = &m;
    }
    let t = r;
    println!("{t}");
}

// The borrowed variable is moved while borrowed (does not compile).
fn move_while_borrowed() {
    let a = String::from("a");
    let r = &a;
    let b = a;
    println!("{r} {b}");
}

struct Holder<'a>(&'a String);

impl Drop for Holder<'_> {
    fn drop(&mut self) {
        println!("{}", self.0);
    }
}

// The borrow is used by the destructor of `_h` (does not compile).
fn drop_use() {
    let _h;
    {
        let s = String::from("s");
        _h = Holder(&s);
    }
}

// NLL accepts this because `first` is dead before the mutable borrow (compiles).
fn nll() {
    let mut v = vec![1];
    let first = &v[0];
    println!("{first}");
    v.push(2);
}

// A borrow held by a struct and used through another reference (compiles).
fn struct_borrow() {
    let s = String::from("s");
    let h = Holder(&s);
    let t = &h;
    println!("{}", t.0);
}

// A guard borrows the mutex until the end of the block (compiles).
fn guard() {
    let m = std::sync::Mutex::new(0);
    let g = m.lock().unwrap();
    println!("{}", *g);
}

// A borrow of `s` from one iteration is used in the next iteration, where `s`
// holds a new value (does not compile).
fn in_loop() {
    let mut prev: Option<&String> = None;
    for _ in 0..2 {
        let mut s = String::new();
        s.push('a');
        if let Some(p) = prev {
            println!("{p}");
        }
        prev = Some(&s);
    }
}

// `v` is initialized and borrowed on only one branch (compiles).
fn cond_init(c: bool) {
    let other = String::from("o");
    let v;
    let r;
    if c {
        v = String::from("v");
        r = &v;
    } else {
        r = &other;
    }
    println!("{r}");
}

// `a` is moved on only one branch while borrowed (does not compile).
fn cond_move(c: bool) {
    let a = String::from("a");
    let r = &a;
    if c {
        drop(a);
    }
    println!("{r}");
}

// `a` is moved while borrowed and then reassigned (does not compile).
fn reinit() {
    let mut a = String::from("a");
    let r = &a;
    drop(a);
    a = String::from("b");
    println!("{r} {a}");
}
