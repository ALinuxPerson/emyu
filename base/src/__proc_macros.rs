pub extern crate inventory;

use hashbrown::HashMap;
use std::iter::Iterator;
use std::sync::LazyLock;

static MODELS: LazyLock<HashMap<&'static str, &'static ModelDescriptor>> = LazyLock::new(|| {
    inventory::iter::<ModelDescriptor>()
        .map(|md| (md.name, md))
        .collect()
});

fn models() -> &'static HashMap<&'static str, &'static ModelDescriptor> {
    &MODELS
}

pub enum Visibility {
    Public,
    Inherited,
}

pub struct ModelDescriptor {
    pub name: &'static str, // `FooModel`
    pub path: &'static str, // `crate::foo::bar::FooModel`
    pub root: bool,
    pub new_vis: Visibility,
    pub methods: &'static [MethodDescriptor],
}

impl ModelDescriptor {
    pub fn root() -> Option<&'static Self> {
        inventory::iter::<Self>().find(|md| md.root)
    }

    pub fn models(&self) -> impl Iterator<Item = &'static ModelDescriptor> {
        self.methods.iter().filter_map(|md| {
            if let MethodDescriptorVariant::Getter {
                ret_ty: ReturnTy::Model(name),
            } = md.variant
            {
                models().get(name).copied()
            } else {
                None
            }
        })
    }
}

pub enum Ty {
    Int(IntKind),
    Float(FloatLength),
    Bool,
    Char,
    Unit,
    String,
    Array(&'static Self, usize),
    Vec(&'static Self),
    Bytes,
    VecDeque(&'static Self),
    HashMap(&'static Self, &'static Ty),
    HashSet(&'static Self),
    BTreeMap(&'static Self, &'static Ty),
    BTreeSet(&'static Self),
    Option(&'static Self),
    Result(&'static Self, &'static Ty),
    SystemTime,
    Instant,
    Duration,
    Unknown(&'static str, &'static [&'static Self]),
}

impl Ty {
    pub fn from_unknown(name: &'static str, generics: &'static [&'static Ty]) -> Self {
        match (name, generics) {
            ("u8", &[]) => Self::Int(IntKind::U8),
            ("u16", &[]) => Self::Int(IntKind::U16),
            ("u32", &[]) => Self::Int(IntKind::U32),
            ("u64", &[]) => Self::Int(IntKind::U64),
            ("u128", &[]) => Self::Int(IntKind::U128),
            ("usize", &[]) => Self::Int(IntKind::USIZE),
            ("i8", &[]) => Self::Int(IntKind::I8),
            ("i16", &[]) => Self::Int(IntKind::I16),
            ("i32", &[]) => Self::Int(IntKind::I32),
            ("i64", &[]) => Self::Int(IntKind::I64),
            ("i128", &[]) => Self::Int(IntKind::I128),
            ("isize", &[]) => Self::Int(IntKind::ISIZE),
            ("f32", &[]) => Self::Float(FloatLength::_32),
            ("f64", &[]) => Self::Float(FloatLength::_64),
            ("bool", &[]) => Self::Bool,
            ("char", &[]) => Self::Char,
            ("()", &[]) => Self::Unit,
            ("String", &[]) => Self::String,
            ("Vec", &[Self::Int(IntKind::U8)]) => Self::Bytes,
            ("Vec", &[t]) => Self::Vec(t),
            ("VecDeque", &[t]) => Self::VecDeque(t),
            ("HashMap", &[k, v]) => Self::HashMap(k, v),
            ("HashSet", &[t]) => Self::HashSet(t),
            ("BTreeMap", &[k, v]) => Self::BTreeMap(k, v),
            ("BTreeSet", &[t]) => Self::BTreeSet(t),
            ("Option", &[t]) => Self::Option(t),
            ("Result", &[t, e]) => Self::Result(t, e),
            ("SystemTime", &[]) => Self::SystemTime,
            ("Instant", &[]) => Self::Instant,
            ("Duration", &[]) => Self::Duration,
            _ => Self::Unknown(name, generics),
        }
    }

    pub fn to_unknown(&self) -> (&'static str, Option<Generics>) {
        match self {
            Self::Int(IntKind::U8) => ("u8", None),
            Self::Int(IntKind::U16) => ("u16", None),
            Self::Int(IntKind::U32) => ("u32", None),
            Self::Int(IntKind::U64) => ("u64", None),
            Self::Int(IntKind::U128) => ("u128", None),
            Self::Int(IntKind::USIZE) => ("usize", None),
            Self::Int(IntKind::I8) => ("i8", None),
            Self::Int(IntKind::I16) => ("i16", None),
            Self::Int(IntKind::I32) => ("i32", None),
            Self::Int(IntKind::I64) => ("i64", None),
            Self::Int(IntKind::I128) => ("i128", None),
            Self::Int(IntKind::ISIZE) => ("isize", None),
            Self::Float(FloatLength::_32) => ("f32", None),
            Self::Float(FloatLength::_64) => ("f64", None),
            Self::Bool => ("bool", None),
            Self::Char => ("char", None),
            Self::Unit => ("()", None),
            Self::String => ("String", None),
            Self::Array(ty, len) => todo!(),
            Self::Vec(t) => ("Vec", Some(Generics::One(t))),
            Self::Bytes => ("Vec", Some(Generics::One(&Self::Int(IntKind::U8)))),
            Self::VecDeque(t) => ("VecDeque", Some(Generics::One(t))),
            Self::HashMap(k, v) => ("HashMap", Some(Generics::Two(k, v))),
            Self::HashSet(t) => ("HashSet", Some(Generics::One(t))),
            Self::BTreeMap(k, v) => ("BTreeMap", Some(Generics::Two(k, v))),
            Self::BTreeSet(t) => ("BTreeSet", Some(Generics::One(t))),
            Self::Option(t) => ("Option", Some(Generics::One(t))),
            Self::Result(t, e) => ("Result", Some(Generics::Two(t, e))),
            Self::SystemTime => ("SystemTime", None),
            Self::Instant => ("Instant", None),
            Self::Duration => ("Duration", None),
            Self::Unknown(name, generics) => (name, Some(Generics::Many(generics))),
        }
    }
}

pub enum Generics {
    One(&'static Ty),
    Two(&'static Ty, &'static Ty),
    Many(&'static [&'static Ty]),
}

#[derive(Eq, PartialEq)]
pub enum IntKind {
    Unsigned(IntLength),
    Signed(IntLength),
}

impl IntKind {
    pub const U8: Self = Self::Unsigned(IntLength::_8);
    pub const U16: Self = Self::Unsigned(IntLength::_16);
    pub const U32: Self = Self::Unsigned(IntLength::_32);
    pub const U64: Self = Self::Unsigned(IntLength::_64);
    pub const U128: Self = Self::Unsigned(IntLength::_128);
    pub const USIZE: Self = Self::Unsigned(IntLength::Size);
}

impl IntKind {
    pub const I8: Self = Self::Signed(IntLength::_8);
    pub const I16: Self = Self::Signed(IntLength::_16);
    pub const I32: Self = Self::Signed(IntLength::_32);
    pub const I64: Self = Self::Signed(IntLength::_64);
    pub const I128: Self = Self::Signed(IntLength::_128);
    pub const ISIZE: Self = Self::Signed(IntLength::Size);
}

#[derive(Eq, PartialEq)]
pub enum IntLength {
    _8,
    _16,
    _32,
    _64,
    _128,
    Size,
}

pub enum FloatLength {
    _32,
    _64,
}

pub struct MethodDescriptor {
    pub vis: Visibility,
    pub name: &'static str,
    pub variant: MethodDescriptorVariant,
}

pub enum MethodDescriptorVariant {
    Updater { args: &'static [MethodArg] },
    Getter { ret_ty: ReturnTy },
}

pub struct MethodArg {
    pub name: &'static str,
    pub ty: Ty,
}

pub enum ReturnTy {
    Ty(Ty),
    Model(&'static str),
}

inventory::collect!(ModelDescriptor);
