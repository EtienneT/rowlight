# Licenses and notices

Rowlight's code is [MIT licensed](../LICENSE). Third-party code and assets keep
their own licenses. Downloads include [THIRD_PARTY_NOTICES.html](../THIRD_PARTY_NOTICES.html),
with complete license texts, copyright notices and links to exact crate source
archives. Embedded Lucide and Feather icon attribution is included separately.
System graphics libraries are provided by the operating system.

The dependency graph includes unmodified MPL-2.0 components. Their exact
published source archives are linked in the notices. Do not remove those links
or the license texts when redistributing a binary.

After changing dependencies or Cargo.lock, regenerate notices with
[cargo-about](https://github.com/EmbarkStudios/cargo-about) 0.9.2 and Python 3:

```sh
cargo install --locked cargo-about --version 0.9.2
python3 scripts/generate-notices.py
```

Review the selected licenses and any new attribution requirements. The generator
covers the maintained Linux/Windows targets with cloud enabled, omits local Cargo
paths, and includes additional NOTICE files. Packaging checks the Cargo.lock hash
and refuses stale notices. Core packages use a subset of the same dependencies.

Experimental macOS redistribution needs a separate Mac dependency/asset review;
the maintained-target notice inventory does not claim to cover it.
