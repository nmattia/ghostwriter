# Hacking

Build the code:

```
cargo build
```

By default the code is built for embedded.

Test the twine code:

```
cargo test --target "$(rustc -vV | grep host | awk '{ print $2; }')" -p twine
```

This will override the embedded target and ensure the tests are run on the host.
