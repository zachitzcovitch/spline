// The problem-type base is checked where it's written, the same way
// `check_problem_types` checks it at run time.

mod no_scheme {
    #[splinekit::contract]
    #[http(problem_types = "example.com/errors")]
    pub trait Users {}
}

mod trailing_slash {
    #[splinekit::contract]
    #[http(problem_types = "https://example.com/errors/")]
    pub trait Users {}
}

mod query {
    #[splinekit::contract]
    #[http(problem_types = "https://example.com/errors?v=1")]
    pub trait Users {}
}

mod duplicate {
    #[splinekit::contract]
    #[http(problem_types = "https://example.com/errors", problem_types = "https://example.com/other")]
    pub trait Users {}
}

mod unknown_option {
    #[splinekit::contract]
    #[http(base = "/users")]
    pub trait Users {}
}

fn main() {}
