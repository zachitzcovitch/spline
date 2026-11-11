use std::collections::{BTreeMap, HashMap};

use crate::{Context, Error, MissingField, Name, OptionSchema, Result, Schema, Shape, TypeInfo};

macro_rules! scalar {
    ($ty:ty, $name:literal, $shape:expr) => {
        impl Schema for $ty {
            fn info() -> TypeInfo {
                TypeInfo::inline($name)
            }

            fn describe(_: &mut Context<'_>) -> Result<Shape> {
                Ok($shape)
            }
        }
    };
}

scalar!((), "core.Unit", Shape::Null);
scalar!(bool, "core.Bool", Shape::Boolean);
scalar!(String, "core.String", Shape::String);
scalar!(
    u8,
    "core.U8",
    Shape::Integer {
        signed: false,
        bits: 8
    }
);
scalar!(
    u16,
    "core.U16",
    Shape::Integer {
        signed: false,
        bits: 16
    }
);
scalar!(
    u32,
    "core.U32",
    Shape::Integer {
        signed: false,
        bits: 32
    }
);
scalar!(
    i8,
    "core.I8",
    Shape::Integer {
        signed: true,
        bits: 8
    }
);
scalar!(
    i16,
    "core.I16",
    Shape::Integer {
        signed: true,
        bits: 16
    }
);
scalar!(
    i32,
    "core.I32",
    Shape::Integer {
        signed: true,
        bits: 32
    }
);

macro_rules! unsupported {
    ($($ty:ty),* $(,)?) => {$(
        impl Schema for $ty {
            fn info() -> TypeInfo {
                TypeInfo::inline(concat!("core.", stringify!($ty)))
            }

            fn describe(_: &mut Context<'_>) -> Result<Shape> {
                Err(Error::Unsupported(concat!(
                    stringify!($ty), " requires an explicit numeric/codec policy"
                ).into()))
            }
        }
    )*};
}

unsupported!(u64, i64, u128, i128, usize, isize, f32, f64);

impl<T: Schema> Schema for Option<T> {
    fn info() -> TypeInfo {
        TypeInfo::inline(Name::applied("core.Option", vec![T::info().name])).accepts_missing()
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        Ok(Shape::Union(vec![cx.schema::<T>()?, Shape::Null]))
    }

    fn missing_field() -> MissingField {
        MissingField::Accepted
    }
}

impl<T: Schema> OptionSchema for Option<T> {
    type Value = T;
}

impl<T: Schema> Schema for Vec<T> {
    fn info() -> TypeInfo {
        TypeInfo::inline(Name::applied("core.Vec", vec![T::info().name]))
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        Ok(Shape::Array(Box::new(cx.schema::<T>()?)))
    }

    fn missing_field() -> MissingField {
        MissingField::Rejected
    }
}

impl<T: Schema> Schema for Box<T> {
    fn info() -> TypeInfo {
        let value = T::info();
        let mut info = TypeInfo::inline(Name::applied("core.Box", vec![value.name]));
        info.missing = T::missing_field();
        info
    }

    fn describe(cx: &mut Context<'_>) -> Result<Shape> {
        cx.schema::<T>()
    }

    fn missing_field() -> MissingField {
        T::missing_field()
    }
}

macro_rules! map {
    ($ty:ident, $name:literal) => {
        impl<T: Schema> Schema for $ty<String, T> {
            fn info() -> TypeInfo {
                TypeInfo::inline(Name::applied($name, vec![T::info().name]))
            }

            fn describe(cx: &mut Context<'_>) -> Result<Shape> {
                Ok(Shape::Map(Box::new(cx.schema::<T>()?)))
            }

            fn missing_field() -> MissingField {
                MissingField::Rejected
            }
        }
    };
}

map!(BTreeMap, "core.BTreeMap");
map!(HashMap, "core.HashMap");
