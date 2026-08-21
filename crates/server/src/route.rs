#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

impl Method {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Sync,
    Items,
    Diff,
    Plan,
    Apply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    method: Method,
    path: &'static str,
    commands: &'static [&'static str],
    target: Target,
}

impl Route {
    #[must_use]
    pub const fn method(&self) -> Method {
        self.method
    }

    #[must_use]
    pub const fn path(&self) -> &'static str {
        self.path
    }

    #[must_use]
    pub const fn commands(&self) -> &'static [&'static str] {
        self.commands
    }

    pub(crate) const fn target(&self) -> Target {
        self.target
    }
}

pub const ROUTES: [Route; 5] = [
    Route {
        method: Method::Post,
        path: "/sync",
        commands: &["sync"],
        target: Target::Sync,
    },
    Route {
        method: Method::Get,
        path: "/items",
        commands: &["ls", "find"],
        target: Target::Items,
    },
    Route {
        method: Method::Get,
        path: "/diff",
        commands: &["diff"],
        target: Target::Diff,
    },
    Route {
        method: Method::Post,
        path: "/plan",
        commands: &["plan"],
        target: Target::Plan,
    },
    Route {
        method: Method::Post,
        path: "/apply",
        commands: &["apply"],
        target: Target::Apply,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Match {
    Found(Route),
    OtherMethod,
    Absent,
}

pub(crate) fn look_up(method: Option<Method>, path: &str) -> Match {
    let mut seen = Match::Absent;
    for route in ROUTES {
        if route.path != path {
            continue;
        }
        if Some(route.method) == method {
            return Match::Found(route);
        }
        seen = Match::OtherMethod;
    }
    seen
}
