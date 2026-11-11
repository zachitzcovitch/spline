# Wire types

Every field crosses the wire as JSON, so each Rust type has one JSON form that
the Rust client, the TypeScript client and OpenAPI all agree on. This guide lists
the built-in forms and how a type declares its own.

## Built-in types

| Rust | JSON | TypeScript | OpenAPI |
| --- | --- | --- | --- |
| `bool`, `String`, `()` | boolean, string, null | `boolean`, `string`, `null` | same types |
| `u8`–`u32`, `i8`–`i32` | integer within the type's range | `number` | `integer` with `minimum`/`maximum` |
| `f32`, `f64` | finite number | `number` | `number`, format `float`/`double` |
| `Option`, `Vec`, `Box`, string-keyed `BTreeMap`/`HashMap` | as their contents | as their contents | as their contents |

Floats have no JSON form for `NaN` or infinity: Serde writes them as `null`,
which no float field accepts back, and both clients refuse them.

`u64`, `i64`, `u128`, `i128`, `usize` and `isize` have no default form.
JavaScript numbers hold integers exactly only up to 2^53 − 1, so a bare field of
these types is a registration error. Give it an explicit form (below).

## Formatted strings and safe integers

Some JSON strings and numbers carry more than their JSON type: a UUID, a date, an
exact decimal, a 64-bit integer written as digits. The schema graph records these
as formatted strings (`Shape::Formatted`) and JavaScript-safe integers, so that:

- OpenAPI describes them: `format: uuid`, `date-time`, `date` or `uri`, and a
  `pattern` for decimal and integer strings;
- the generated TypeScript client checks them, in arguments before sending and in
  responses, alongside the other structural checks. Integer strings are range
  checked without `BigInt`;
- Rust clients rely on Serde: the Rust type's own decoding enforces the format.

| Wire description | JSON form | TypeScript |
| --- | --- | --- |
| `splinekit::wire::Uuid` | hyphenated UUID string | `string` |
| `splinekit::wire::DateTime` | RFC 3339 date-time with offset or `Z` | `string` |
| `splinekit::wire::Date` | RFC 3339 full date, `YYYY-MM-DD` | `string` |
| `splinekit::wire::Uri` | absolute URI string | `string` |
| `splinekit::wire::Decimal` | exact decimal string, such as `-12.50` | `string` |
| `splinekit::wire::IntegerString<u64>` (also `i64`, `u128`, `i128`, `usize`, `isize`) | base-10 digits within the type's range | `string` |
| `splinekit::wire::SafeInteger<i64>` (same types) | JSON integer within ±(2^53 − 1) | `number` |

## Declaring a type's wire form

A type whose Serde form isn't its Rust shape names its wire description with
`#[splinekit(schema = "...")]`, on the type or on a field:

```rust
#[derive(serde::Serialize, serde::Deserialize, splinekit::Schema)]
#[serde(try_from = "String", into = "String")]
#[splinekit(schema = "splinekit::wire::Decimal")]
pub struct Usd(i64);

#[derive(serde::Serialize, serde::Deserialize, splinekit::Schema)]
pub struct Invoice {
    #[splinekit(schema = "splinekit::wire::Uuid")]
    pub id: String,
    pub total: Usd,
}
```

The description is a declaration, not a codec: the type's Serde code must produce
and accept exactly the described form. A field description replaces the field
type's own schema, so the field type doesn't need to implement `Schema`.
