# Platform support

| Platform | Status | Distribution |
| --- | --- | --- |
| Linux x64 | Maintained | Archive with a per-user installer; Ubuntu 24.04 / glibc 2.39 baseline |
| Windows x64 | Maintained | Per-user installer and portable ZIP; Windows 10 or later |
| macOS | Experimental, community contributions welcome | Source and existing native packaging scripts only |
| Other architectures | Unverified | Source contributions welcome |

Linux needs a Wayland or X11 session and a Vulkan graphics driver. Windows needs
a DirectX 11 graphics driver. A compatible CPU and system libraries alone do not
establish desktop compatibility; see [native release checks](../packaging/TESTING.md).

The maintainer currently has Linux and Windows hardware. macOS is outside the
official release matrix: no Mac downloads, native test guarantees or signing
commitment. The existing code and scripts remain available rather than removing
the path for future contributors.

To make macOS maintained, a contributor needs to take responsibility for native
builds, file opening/associations, appearance, keyboard navigation and release
testing on the architectures they support. Signing and notarization can be
planned with that maintainer when Mac downloads are introduced.

The first Linux/Windows release is an **unsigned alpha**. Windows may show an
unrecognized-publisher warning. Downloads include SHA-256 checksums to verify
integrity; a checksum is not a publisher signature. Signing can be added later.
