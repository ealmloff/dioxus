#![allow(dead_code)]
//! Composable view builder with a compile-time flat template AND a fine-grained
//! update path: `rebuild` walks only the dynamic holes, patches the ones that
//! changed, and never touches static content. The diff stays TYPED end to end —
//! a `const DYN_COUNT` per view type lets the parent delete the recursion into
//! any all-static subtree at compile time (no erasure into a runtime array).

use std::marker::PhantomData;

// ===== efficient flat op template ==========================================
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    Enter { skip: u16, ns: bool },
    Exit,
    Attr,
    Text,
    Static(u16),
    Dyn(u16),
}
#[derive(Clone, Copy, Debug)]
pub struct Span {
    off: u16,
    len: u16,
}

const CAP: usize = 64;
#[derive(Clone, Copy)]
pub enum RawOp {
    Open(&'static str),
    Close,
    SAttr(&'static str, &'static str),
    DAttr(&'static str),
    SText(&'static str),
    DChild,
}
#[derive(Clone, Copy)]
pub struct RawTape {
    ops: [RawOp; CAP],
    len: usize,
}
impl RawTape {
    pub const fn new() -> Self {
        Self {
            ops: [RawOp::Close; CAP],
            len: 0,
        }
    }
    pub const fn push(mut self, op: RawOp) -> Self {
        self.ops[self.len] = op;
        self.len += 1;
        self
    }
    pub const fn concat(mut self, o: &RawTape) -> Self {
        let mut i = 0;
        while i < o.len {
            self.ops[self.len] = o.ops[i];
            self.len += 1;
            i += 1;
        }
        self
    }
}

#[derive(Clone, Copy)]
pub struct FlatTemplate {
    ops: [Op; CAP],
    n: usize,
    blob: [u8; CAP],
    bn: usize,
    spans: [Span; CAP],
    sn: usize,
    dyns: [u16; CAP],
    dynn: usize,
}
impl FlatTemplate {
    const fn empty() -> Self {
        Self {
            ops: [Op::Exit; CAP],
            n: 0,
            blob: [0; CAP],
            bn: 0,
            spans: [Span { off: 0, len: 0 }; CAP],
            sn: 0,
            dyns: [0; CAP],
            dynn: 0,
        }
    }
    const fn push(mut self, op: Op) -> Self {
        self.ops[self.n] = op;
        self.n += 1;
        self
    }
    const fn intern(mut self, s: &str) -> (Self, u16) {
        let sb = s.as_bytes();
        let mut k = 0;
        while k < self.sn {
            let sp = self.spans[k];
            if sp.len as usize == sb.len() {
                let mut i = 0;
                let mut eq = true;
                while i < sb.len() {
                    if self.blob[sp.off as usize + i] != sb[i] {
                        eq = false;
                        break;
                    }
                    i += 1;
                }
                if eq {
                    return (self, k as u16);
                }
            }
            k += 1;
        }
        let off = self.bn;
        let mut i = 0;
        while i < sb.len() {
            self.blob[off + i] = sb[i];
            i += 1;
        }
        self.bn += sb.len();
        let idx = self.sn;
        self.spans[idx] = Span {
            off: off as u16,
            len: sb.len() as u16,
        };
        self.sn += 1;
        (self, idx as u16)
    }
    const fn push_static(self, s: &str) -> Self {
        let (t, i) = self.intern(s);
        t.push(Op::Static(i))
    }
    const fn push_dyn(mut self) -> Self {
        let id = self.dynn as u16;
        self.dyns[self.dynn] = self.n as u16;
        self.dynn += 1;
        self.push(Op::Dyn(id))
    }
    fn str_at(&self, i: u16) -> &str {
        let sp = self.spans[i as usize];
        core::str::from_utf8(&self.blob[sp.off as usize..sp.off as usize + sp.len as usize])
            .unwrap()
    }
}
const fn drive(raw: &RawTape) -> FlatTemplate {
    let mut t = FlatTemplate::empty();
    let mut stack = [0usize; CAP];
    let mut sp = 0;
    let mut k = 0;
    while k < raw.len {
        match raw.ops[k] {
            RawOp::Open(tag) => {
                stack[sp] = t.n;
                sp += 1;
                t = t.push(Op::Enter { skip: 0, ns: false });
                t = t.push_static(tag);
            }
            RawOp::Close => {
                t = t.push(Op::Exit);
                sp -= 1;
                let o = stack[sp];
                let skip = (t.n - o) as u16;
                if let Op::Enter { ns, .. } = t.ops[o] {
                    t.ops[o] = Op::Enter { skip, ns };
                }
            }
            RawOp::SAttr(n, v) => {
                t = t.push(Op::Attr);
                t = t.push_static(n);
                t = t.push_static(v);
            }
            RawOp::DAttr(n) => {
                t = t.push(Op::Attr);
                t = t.push_static(n);
                t = t.push_dyn();
            }
            RawOp::SText(s) => {
                t = t.push(Op::Text);
                t = t.push_static(s);
            }
            RawOp::DChild => {
                t = t.push(Op::Text);
                t = t.push_dyn();
            }
        }
        k += 1;
    }
    t
}

// ===== the three composable halves =========================================
pub trait Raw {
    const RAW: RawTape;
}
pub trait Built {
    const TEMPLATE: FlatTemplate;
}
impl<V: Raw> Built for V {
    const TEMPLATE: FlatTemplate = drive(&V::RAW);
}

pub trait Render: Sized {
    type State: Collect;
    const DYN_COUNT: usize; // number of dynamic holes in this subtree (compile-time)
    const NODES: usize; // total nodes incl. static (for the demo's visit accounting)
    fn build(self) -> Self::State;
    fn rebuild(self, old: &mut Self::State, cx: &mut Patcher);
}
pub trait Collect {
    fn collect<'a>(&'a self, out: &mut Vec<Dyn<'a>>);
}
pub trait Mountable {
    fn render(&self) -> String;
}
pub enum Dyn<'a> {
    Attr(&'a str),
    Child(&'a dyn Mountable),
}

#[derive(Debug)]
pub struct Patch {
    slot: usize,
    tape_pos: u16,
    value: String,
}
pub struct Patcher<'a> {
    slot: usize,
    positions: &'a [u16],
    pub patches: Vec<Patch>,
    pub visits: usize,
}
impl<'a> Patcher<'a> {
    fn new(positions: &'a [u16]) -> Self {
        Self {
            slot: 0,
            positions,
            patches: Vec::new(),
            visits: 0,
        }
    }
    fn check(&mut self, new: &str, old: &mut String) {
        let i = self.slot;
        self.slot += 1;
        if new != old.as_str() {
            self.patches.push(Patch {
                slot: i,
                tape_pos: self.positions[i],
                value: new.to_string(),
            });
            old.clear();
            old.push_str(new);
        }
    }
}

// ----- leaves --------------------------------------------------------------
impl Raw for () {
    const RAW: RawTape = RawTape::new();
}
impl Render for () {
    type State = ();
    const DYN_COUNT: usize = 0;
    const NODES: usize = 1;
    fn build(self) {}
    fn rebuild(self, _: &mut (), _: &mut Patcher) {}
}
impl Collect for () {
    fn collect<'a>(&'a self, _: &mut Vec<Dyn<'a>>) {}
}

macro_rules! text {
    ($n:ident, $s:literal) => {
        pub struct $n;
        impl Raw for $n {
            const RAW: RawTape = RawTape::new().push(RawOp::SText($s));
        }
        impl Render for $n {
            type State = ();
            const DYN_COUNT: usize = 0;
            const NODES: usize = 1;
            fn build(self) {}
            fn rebuild(self, _: &mut (), _: &mut Patcher) {}
        }
    };
}
macro_rules! tag {
    ($n:ident, $s:literal) => {
        pub struct $n;
        impl TagName for $n {
            const NAME: &'static str = $s;
        }
    };
}
macro_rules! attr {
    ($n:ident, $k:literal, $v:literal) => {
        pub struct $n;
        impl Raw for $n {
            const RAW: RawTape = RawTape::new().push(RawOp::SAttr($k, $v));
        }
        impl Render for $n {
            type State = ();
            const DYN_COUNT: usize = 0;
            const NODES: usize = 1;
            fn build(self) {}
            fn rebuild(self, _: &mut (), _: &mut Patcher) {}
        }
    };
}
macro_rules! attr_name {
    ($n:ident, $s:literal) => {
        pub struct $n;
        impl AttrName for $n {
            const NAME: &'static str = $s;
        }
    };
}
pub trait TagName {
    const NAME: &'static str;
}
pub trait AttrName {
    const NAME: &'static str;
}

// dynamic text child
pub struct Dynamic(pub String);
pub fn dynamic(s: impl Into<String>) -> Dynamic {
    Dynamic(s.into())
}
impl Raw for Dynamic {
    const RAW: RawTape = RawTape::new().push(RawOp::DChild);
}
pub struct TextState(String);
impl Render for Dynamic {
    type State = TextState;
    const DYN_COUNT: usize = 1;
    const NODES: usize = 1;
    fn build(self) -> TextState {
        TextState(self.0)
    }
    fn rebuild(self, old: &mut TextState, cx: &mut Patcher) {
        cx.visits += 1;
        cx.check(&self.0, &mut old.0);
    }
}
impl Collect for TextState {
    fn collect<'a>(&'a self, out: &mut Vec<Dyn<'a>>) {
        out.push(Dyn::Child(self));
    }
}
impl Mountable for TextState {
    fn render(&self) -> String {
        self.0.clone()
    }
}

// dynamic attribute value
pub struct DynAttr<Name>(pub String, PhantomData<Name>);
pub fn attr_dyn<Name>(v: impl Into<String>) -> DynAttr<Name> {
    DynAttr(v.into(), PhantomData)
}
impl<Name: AttrName> Raw for DynAttr<Name> {
    const RAW: RawTape = RawTape::new().push(RawOp::DAttr(Name::NAME));
}
pub struct AttrState(String);
impl<Name: AttrName> Render for DynAttr<Name> {
    type State = AttrState;
    const DYN_COUNT: usize = 1;
    const NODES: usize = 1;
    fn build(self) -> AttrState {
        AttrState(self.0)
    }
    fn rebuild(self, old: &mut AttrState, cx: &mut Patcher) {
        cx.visits += 1;
        cx.check(&self.0, &mut old.0);
    }
}
impl Collect for AttrState {
    fn collect<'a>(&'a self, out: &mut Vec<Dyn<'a>>) {
        out.push(Dyn::Attr(&self.0));
    }
}

// ----- tuples --------------------------------------------------------------
impl<A: Raw, B: Raw> Raw for (A, B) {
    const RAW: RawTape = A::RAW.concat(&B::RAW);
}
impl<A: Render, B: Render> Render for (A, B) {
    type State = (A::State, B::State);
    const DYN_COUNT: usize = A::DYN_COUNT + B::DYN_COUNT;
    const NODES: usize = 1 + A::NODES + B::NODES;
    fn build(self) -> Self::State {
        (self.0.build(), self.1.build())
    }
    fn rebuild(self, old: &mut Self::State, cx: &mut Patcher) {
        cx.visits += 1;
        if A::DYN_COUNT != 0 {
            self.0.rebuild(&mut old.0, cx);
        } // const guard: static subtree pruned at compile time
        if B::DYN_COUNT != 0 {
            self.1.rebuild(&mut old.1, cx);
        }
    }
}
impl<A: Collect, B: Collect> Collect for (A, B) {
    fn collect<'a>(&'a self, out: &mut Vec<Dyn<'a>>) {
        self.0.collect(out);
        self.1.collect(out);
    }
}

// ----- element + builder ---------------------------------------------------
pub struct El<Tag, At, Ch> {
    attrs: At,
    children: Ch,
    _t: PhantomData<Tag>,
}
pub fn el<Tag>() -> El<Tag, (), ()> {
    El {
        attrs: (),
        children: (),
        _t: PhantomData,
    }
}
impl<Tag, At, Ch> El<Tag, At, Ch> {
    pub fn attr<A>(self, a: A) -> El<Tag, (At, A), Ch> {
        El {
            attrs: (self.attrs, a),
            children: self.children,
            _t: PhantomData,
        }
    }
    pub fn child<C>(self, c: C) -> El<Tag, At, (Ch, C)> {
        El {
            attrs: self.attrs,
            children: (self.children, c),
            _t: PhantomData,
        }
    }
}
impl<Tag: TagName, At: Raw, Ch: Raw> Raw for El<Tag, At, Ch> {
    const RAW: RawTape = RawTape::new()
        .push(RawOp::Open(Tag::NAME))
        .concat(&At::RAW)
        .concat(&Ch::RAW)
        .push(RawOp::Close);
}
pub struct ElState<A, C> {
    attrs: A,
    children: C,
}
impl<Tag: TagName, At: Render, Ch: Render> Render for El<Tag, At, Ch> {
    type State = ElState<At::State, Ch::State>;
    const DYN_COUNT: usize = At::DYN_COUNT + Ch::DYN_COUNT;
    const NODES: usize = 1 + At::NODES + Ch::NODES;
    fn build(self) -> Self::State {
        ElState {
            attrs: self.attrs.build(),
            children: self.children.build(),
        }
    }
    fn rebuild(self, old: &mut Self::State, cx: &mut Patcher) {
        cx.visits += 1;
        if At::DYN_COUNT != 0 {
            self.attrs.rebuild(&mut old.attrs, cx);
        }
        if Ch::DYN_COUNT != 0 {
            self.children.rebuild(&mut old.children, cx);
        }
    }
}
impl<A: Collect, C: Collect> Collect for ElState<A, C> {
    fn collect<'a>(&'a self, out: &mut Vec<Dyn<'a>>) {
        self.attrs.collect(out);
        self.children.collect(out);
    }
}

// ===== render (full) =======================================================
fn render_flat(t: &FlatTemplate, d: &[Dyn]) -> String {
    fn sidx(op: Op) -> u16 {
        if let Op::Static(x) = op { x } else { panic!() }
    }
    fn walk(t: &FlatTemplate, d: &[Dyn], mut i: usize, end: usize, out: &mut String) {
        while i < end {
            match t.ops[i] {
                Op::Enter { skip, ns } => {
                    let tag = t.str_at(sidx(t.ops[i + 1]));
                    out.push('<');
                    out.push_str(tag);
                    let mut j = i + 2;
                    if ns {
                        j += 1;
                    }
                    while matches!(t.ops[j], Op::Attr) {
                        out.push(' ');
                        out.push_str(t.str_at(sidx(t.ops[j + 1])));
                        out.push_str("=\"");
                        match t.ops[j + 2] {
                            Op::Static(x) => out.push_str(t.str_at(x)),
                            Op::Dyn(id) => {
                                if let Dyn::Attr(s) = &d[id as usize] {
                                    out.push_str(s)
                                }
                            }
                            _ => {}
                        }
                        out.push('"');
                        j += 3;
                    }
                    out.push('>');
                    walk(t, d, j, i + skip as usize - 1, out);
                    out.push_str("</");
                    out.push_str(tag);
                    out.push('>');
                    i += skip as usize;
                }
                Op::Text => {
                    match t.ops[i + 1] {
                        Op::Static(x) => out.push_str(t.str_at(x)),
                        Op::Dyn(id) => {
                            if let Dyn::Child(c) = &d[id as usize] {
                                out.push_str(&c.render())
                            }
                        }
                        _ => {}
                    }
                    i += 2;
                }
                _ => i += 1,
            }
        }
    }
    let mut out = String::new();
    walk(t, d, 0, t.n, &mut out);
    out
}
fn render_state<S: Collect>(state: &S, t: &FlatTemplate) -> String {
    let mut d = Vec::new();
    state.collect(&mut d);
    render_flat(t, &d)
}

// ===========================================================================
tag!(Div, "div");
tag!(H2, "h2");
tag!(P, "p");
tag!(SpanTag, "span");
attr!(CardClass, "class", "card");
attr!(BadgeClass, "class", "badge");
attr!(TitleRole, "data-role", "title");
text!(TitlePrefix, "Title: ");
text!(HelloPrefix, "Hello, ");
attr_name!(StyleName, "style");

// View = Raw + Render: everything a composed view satisfies. Returned as
// `impl View` so call sites never spell out the nested-tuple type.
pub trait View: Raw + Render {}
impl<T: Raw + Render> View for T {}

fn badge(content: impl View) -> impl View {
    el::<SpanTag>().attr(BadgeClass).child(content)
}
// one composable view definition, parameterized by its dynamic values
fn card(style: &str, title: &str, name: &str) -> impl View {
    el::<Div>()
        .attr(CardClass)
        .attr(attr_dyn::<StyleName>(style))
        .child(
            el::<H2>()
                .attr(TitleRole)
                .child(TitlePrefix)
                .child(dynamic(title)),
        )
        .child(
            el::<P>()
                .attr(CardClass)
                .child(HelloPrefix)
                .child(badge(dynamic(name))),
        )
}

// generic helpers read the template and node count off any view's TYPE, so
// the call site never names that type.
fn template_of<V: Raw>(_: &V) -> FlatTemplate {
    <V as Built>::TEMPLATE
}
fn nodes_of<V: Render>(_: &V) -> usize {
    V::NODES
}

fn main() {
    let v1 = card("color: crimson", "Welcome", "Ada");
    let t = template_of(&v1);
    let total = nodes_of(&v1); // read off the type, before build() consumes it
    let mut state = v1.build();
    println!("template: {} ops, holes at {:?}", t.n, &t.dyns[..t.dynn]);
    println!("initial render:\n{}\n", render_state(&state, &t));

    // --- update: only the title changes; style and name are identical -------
    let v2 = card("color: crimson", "Hello there", "Ada");
    let mut cx = Patcher::new(&t.dyns[..t.dynn]);
    v2.rebuild(&mut state, &mut cx);

    println!(
        "rebuild visited {} of {} state nodes; {} patch(es):",
        cx.visits,
        total,
        cx.patches.len()
    );
    for p in &cx.patches {
        println!(
            "  slot {} @ tape op {} -> {:?}",
            p.slot, p.tape_pos, p.value
        );
    }
    println!(
        "\nre-render from patched state:\n{}",
        render_state(&state, &t)
    );
}

// proof the template is a compile-time constant: drive() runs in const context
// on a named view type (no composed type spelled out).
const _: () =
    assert!(<Dynamic as Built>::TEMPLATE.dynn == 1 && <Dynamic as Built>::TEMPLATE.n == 2);
