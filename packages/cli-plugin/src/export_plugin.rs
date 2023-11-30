#[macro_export]
macro_rules! export_plugin {
    ($name:ident) => {
        ::wit_bindgen::generate!({
            inline: "package plugins:main;

interface definitions {
  use imports.{platform};
  use toml.{toml, toml-value};

  /// Get the default layout for the plugin to put
  /// into `Dioxus.html`
  get-default-config: func() -> toml;

  /// Take config from `Dioxus.toml` and apply
  /// to the plugin, returns false if couldn't apply
  apply-config: func(config: toml) -> result;
  
  // Initialize the plugin
  register: func() -> result;

  // Before the app is built
  before-build: func() -> result;

  // After the application is built, before serve
  before-serve: func() -> result;

  // Reload on serve with no hot-reloading(?)
  on-rebuild: func() -> result;

  // Reload on serve with hot-reloading
  on-hot-reload: func();

  /// Check if there is an update to the plugin 
  /// with a given git repo?
  /// returns error if there was error getting git
  /// Some(url) => git clone url
  /// None => No update needed
  /// check-update: func() -> result<option<string>>

  on-watched-paths-change: func(path: list<string>);
}

interface toml {
  resource toml {
    /// Creates a new handle from value
    constructor(value: toml-value);
    /// Clones value from table by handle
    get: func() -> toml-value;
    /// Sets value in table by handle
    set: func(value: toml-value);
    /// Clones the handle, not the value
    clone: func() -> toml;
  }

  variant toml-value {
    %string(string),
    integer(s64),
    float(float64),
    boolean(bool),
    datetime(datetime),
    %array(array),
    %table(table),
  }

  record datetime {
    date: option<date>,
    time: option<time>,
    offset: option<offset>,
  }

  record date {
    year: u16,
    month: u8,
    day: u8,
  }

  record time {
    hour: u8,
    minute: u8, 
    second: u8,
    nanosecond: u32,
  }

  variant offset {
    z,
    custom(tuple<s8,u8>),
  }
  
  type array = list<toml>;
  type table = list<tuple<string, toml>>;
}

interface imports {
  enum platform {
    web,
    desktop,
  }

  get-platform: func() -> platform;

  output-directory: func() -> string;

  refresh-browser-page: func();

  /// Searches through links to only refresh the 
  /// necessary components when changing assets
  refresh-asset: func(old-url: string, new-url: string);

  /// Add path to list of watched paths
  watch-path: func(path: string);

  /// Get list of watched paths
  watched-paths: func() -> list<string>;

  /// Try to remove a path from list of watched paths
  /// returns false if path not in list
  remove-path: func(path: string) -> result;

  log: func(info: string);

}

world plugin-world {
  import imports;
  import toml;
  export definitions;
}
",
            world: "plugin-world",
            exports: {
                world: $name,
                "plugins:main/definitions": $name
            },
        });
    };
}