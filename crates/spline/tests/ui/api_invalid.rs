use spline::api;

api! { mod empty {} }

api! { mod duplicates { users: self::missing, users: self::missing } }

api! { mod reserved { new: self::missing } }

api! { mod unicode { café: self::missing } }

fn main() {}
