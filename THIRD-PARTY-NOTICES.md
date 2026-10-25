# Third-party notices

This file lists every third-party crate pinned in `Cargo.lock`, with its License and the
copyright line that License requires us to reproduce. It exists because `LICENSE` covers
this project's own code (BSD 2-Clause) and says nothing about the crates it links, while
MIT, ISC and the Unicode License all require their notice to travel with a binary that
contains them.

**Nothing here is copyleft.** Every entry is MIT, Apache-2.0, ISC, the Unicode License or
the Unlicense. `RELEASING.md` records the check that keeps it that way.

The table covers the whole resolved graph, including crates that only run at build time or
expand macros. A proc-macro contributes no code to a binary, but it is still part of the
source distribution `Cargo.lock` pins, so listing it costs nothing and leaves nothing to
explain later.

Which entries actually reach a binary: `wikitext` and `wikitext-lua` link nothing at all;
the CLI adds `clap`; `wikitext-convert` adds `html5ever` (and with it `markup5ever`,
`tendril`, `string_cache`, `phf` and `xml5ever`), which the C ABI library, the Node addon and
the WebAssembly module all inherit; the Node addon additionally links `napi`, which reaches
`libloading`, `futures`, `parking_lot` and `convert_case`; the WebAssembly module adds
`wasm-bindgen`.

One algorithm is not a dependency and owes nothing: the civil date conversion in
`crates/wikitext/src/preprocess/functions.rs` follows Howard Hinnant's public-domain date
algorithms.

## Crates

| Crate | Version | License | Copyright |
|---|---|---|---|
| `anstream` | 1.0.0 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `anstyle` | 1.0.14 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `anstyle-parse` | 1.0.0 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `anstyle-query` | 1.1.5 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `anstyle-wincon` | 3.0.11 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `bitflags` | 2.13.2 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| `bumpalo` | 3.20.3 | MIT OR Apache-2.0 | Copyright (c) 2019 Nick Fitzgerald |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| `clap` | 4.6.7 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `clap_builder` | 4.6.7 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `clap_derive` | 4.6.7 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `clap_lex` | 1.1.1 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `colorchoice` | 1.0.5 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `convert_case` | 0.12.0 | MIT | Copyright (c) 2025 rutrum |
| `ctor` | 1.0.13 | Apache-2.0 OR MIT | Authors: Matt Mastracci <matthew@mastracci.com> |
| `fastrand` | 2.5.0 | Apache-2.0 OR MIT | Authors: Stjepan Glavina <stjepang@gmail.com> |
| `futures` | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton |
| `futures-channel` | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton |
| `futures-core` | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton |
| `futures-executor` | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton |
| `futures-io` | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton |
| `futures-macro` | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton |
| `futures-sink` | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton |
| `futures-task` | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton |
| `futures-util` | 0.3.34 | MIT OR Apache-2.0 | Copyright (c) 2016 Alex Crichton |
| `heck` | 0.5.0 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| `html5ever` | 0.39.0 | MIT OR Apache-2.0 | Copyright (c) 2014 The html5ever Project Developers |
| `is_terminal_polyfill` | 1.70.2 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `libc` | 0.2.190 | MIT OR Apache-2.0 | Copyright (c) The Rust Project Developers |
| `libloading` | 0.9.0 | ISC | Copyright © 2015, Simonas Kazlauskas |
| `lock_api` | 0.4.14 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| `log` | 0.4.34 | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers |
| `markup5ever` | 0.39.0 | MIT OR Apache-2.0 | Copyright (c) 2014 The html5ever Project Developers |
| `markup5ever_rcdom` | 0.39.0+unofficial | MIT OR Apache-2.0 | Copyright (c) 2014 The html5ever Project Developers |
| `memchr` | 2.8.3 | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant |
| `napi` | 3.14.2 | MIT | Authors: Nathan Sobo <nathan@github.com>, Yinan Long <lynweklm@gmail.com> |
| `napi-build` | 2.6.0 | MIT | Authors: LongYinan <lynweklm@gmail.com> |
| `napi-derive` | 3.6.12 | MIT | Authors: LongYinan <lynweklm@gmail.com>, Forehalo <forehalo@gmail.com> |
| `napi-derive-backend` | 6.1.5 | MIT | the crate authors and contributors |
| `napi-sys` | 3.4.0 | MIT | Authors: LongYinan <lynweklm@gmail.com> |
| `new_debug_unreachable` | 1.0.6 | MIT | Copyright (c) 2015 Jonathan Reem |
| `nohash-hasher` | 0.2.0 | Apache-2.0 OR MIT | Copyright 2018 Parity Technologies (UK) Ltd. |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 | Authors: Aleksey Kladov <aleksey.kladov@gmail.com> |
| `once_cell_polyfill` | 1.70.2 | MIT OR Apache-2.0 | Copyright (c) Individual contributors |
| `parking_lot` | 0.12.5 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| `parking_lot_core` | 0.9.12 | MIT OR Apache-2.0 | Copyright (c) 2016 The Rust Project Developers |
| `phf` | 0.13.1 | MIT | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| `phf_codegen` | 0.13.1 | MIT | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| `phf_generator` | 0.13.1 | MIT | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| `phf_shared` | 0.13.1 | MIT | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| `pin-project-lite` | 0.2.17 | Apache-2.0 OR MIT | the crate authors and contributors |
| `precomputed-hash` | 0.1.1 | MIT | Copyright (c) 2017 Emilio Cobos Álvarez |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 | Authors: David Tolnay <dtolnay@gmail.com>, Alex Crichton <alex@alexcrichton.com> |
| `quote` | 1.0.47 | MIT OR Apache-2.0 | Authors: David Tolnay <dtolnay@gmail.com> |
| `redox_syscall` | 0.5.18 | MIT | Copyright (c) 2017 Redox OS Developers |
| `rustc-hash` | 2.1.3 | Apache-2.0 OR MIT | Authors: The Rust Project Developers |
| `rustversion` | 1.0.23 | MIT OR Apache-2.0 | Authors: David Tolnay <dtolnay@gmail.com> |
| `scopeguard` | 1.2.0 | MIT OR Apache-2.0 | Copyright (c) 2016-2019 Ulrik Sverdrup "bluss" and scopeguard developers |
| `semver` | 1.0.28 | MIT OR Apache-2.0 | Authors: David Tolnay <dtolnay@gmail.com> |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | Authors: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com> |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 | Authors: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com> |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 | Authors: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com> |
| `siphasher` | 1.0.4 | MIT OR Apache-2.0 | Copyright 2012-2016 The Rust Project Developers. |
| `slab` | 0.4.12 | MIT | Copyright (c) 2019 Carl Lerche |
| `smallvec` | 1.16.3 | MIT OR Apache-2.0 | Copyright (c) 2018 The Servo Project Developers |
| `string_cache` | 0.9.0 | MIT OR Apache-2.0 | Copyright (c) 2012-2013 Mozilla Foundation |
| `string_cache_codegen` | 0.6.1 | MIT OR Apache-2.0 | Copyright (c) 2012-2013 Mozilla Foundation |
| `strsim` | 0.11.1 | MIT | Copyright (c) 2015 Danny Guo |
| `syn` | 3.0.8 | MIT OR Apache-2.0 | Authors: David Tolnay <dtolnay@gmail.com> |
| `tendril` | 0.5.1 | MIT OR Apache-2.0 | Copyright (c) 2015 Keegan McAllister |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | Copyright © 1991-2023 Unicode, Inc. |
| `unicode-segmentation` | 1.13.3 | MIT OR Apache-2.0 | Copyright (c) 2015 The Rust Project Developers |
| `utf8parse` | 0.2.2 | Apache-2.0 OR MIT | Copyright (c) 2016 Joe Wilm |
| `wasm-bindgen` | 0.2.129 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| `wasm-bindgen-macro` | 0.2.129 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| `wasm-bindgen-macro-support` | 0.2.129 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| `wasm-bindgen-shared` | 0.2.129 | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton |
| `web_atoms` | 0.2.6 | MIT OR Apache-2.0 | Copyright (c) 2014 The html5ever Project Developers |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 | Copyright (c) Microsoft Corporation. |
| `xml5ever` | 0.39.0 | MIT OR Apache-2.0 | Copyright (c) 2014 The html5ever Project Developers |

## License texts

Where a crate offers a choice (`MIT OR Apache-2.0` and the like), this project takes the
MIT option unless the crate terms say otherwise; the copyright line for each is above.

### MIT

```text
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Apache License 2.0

```text
                                 Apache License
                           Version 2.0, January 2004
                        http://www.apache.org/licenses/

   TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

   1. Definitions.

      "License" shall mean the terms and conditions for use, reproduction,
      and distribution as defined by Sections 1 through 9 of this document.

      "Licensor" shall mean the copyright owner or entity authorized by
      the copyright owner that is granting the License.

      "Legal Entity" shall mean the union of the acting entity and all
      other entities that control, are controlled by, or are under common
      control with that entity. For the purposes of this definition,
      "control" means (i) the power, direct or indirect, to cause the
      direction or management of such entity, whether by contract or
      otherwise, or (ii) ownership of fifty percent (50%) or more of the
      outstanding shares, or (iii) beneficial ownership of such entity.

      "You" (or "Your") shall mean an individual or Legal Entity
      exercising permissions granted by this License.

      "Source" form shall mean the preferred form for making modifications,
      including but not limited to software source code, documentation
      source, and configuration files.

      "Object" form shall mean any form resulting from mechanical
      transformation or translation of a Source form, including but
      not limited to compiled object code, generated documentation,
      and conversions to other media types.

      "Work" shall mean the work of authorship, whether in Source or
      Object form, made available under the License, as indicated by a
      copyright notice that is included in or attached to the work
      (an example is provided in the Appendix below).

      "Derivative Works" shall mean any work, whether in Source or Object
      form, that is based on (or derived from) the Work and for which the
      editorial revisions, annotations, elaborations, or other modifications
      represent, as a whole, an original work of authorship. For the purposes
      of this License, Derivative Works shall not include works that remain
      separable from, or merely link (or bind by name) to the interfaces of,
      the Work and Derivative Works thereof.

      "Contribution" shall mean any work of authorship, including
      the original version of the Work and any modifications or additions
      to that Work or Derivative Works thereof, that is intentionally
      submitted to Licensor for inclusion in the Work by the copyright owner
      or by an individual or Legal Entity authorized to submit on behalf of
      the copyright owner. For the purposes of this definition, "submitted"
      means any form of electronic, verbal, or written communication sent
      to the Licensor or its representatives, including but not limited to
      communication on electronic mailing lists, source code control systems,
      and issue tracking systems that are managed by, or on behalf of, the
      Licensor for the purpose of discussing and improving the Work, but
      excluding communication that is conspicuously marked or otherwise
      designated in writing by the copyright owner as "Not a Contribution."

      "Contributor" shall mean Licensor and any individual or Legal Entity
      on behalf of whom a Contribution has been received by Licensor and
      subsequently incorporated within the Work.

   2. Grant of Copyright License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      copyright license to reproduce, prepare Derivative Works of,
      publicly display, publicly perform, sublicense, and distribute the
      Work and such Derivative Works in Source or Object form.

   3. Grant of Patent License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      (except as stated in this section) patent license to make, have made,
      use, offer to sell, sell, import, and otherwise transfer the Work,
      where such license applies only to those patent claims licensable
      by such Contributor that are necessarily infringed by their
      Contribution(s) alone or by combination of their Contribution(s)
      with the Work to which such Contribution(s) was submitted. If You
      institute patent litigation against any entity (including a
      cross-claim or counterclaim in a lawsuit) alleging that the Work
      or a Contribution incorporated within the Work constitutes direct
      or contributory patent infringement, then any patent licenses
      granted to You under this License for that Work shall terminate
      as of the date such litigation is filed.

   4. Redistribution. You may reproduce and distribute copies of the
      Work or Derivative Works thereof in any medium, with or without
      modifications, and in Source or Object form, provided that You
      meet the following conditions:

      (a) You must give any other recipients of the Work or
          Derivative Works a copy of this License; and

      (b) You must cause any modified files to carry prominent notices
          stating that You changed the files; and

      (c) You must retain, in the Source form of any Derivative Works
          that You distribute, all copyright, patent, trademark, and
          attribution notices from the Source form of the Work,
          excluding those notices that do not pertain to any part of
          the Derivative Works; and

      (d) If the Work includes a "NOTICE" text file as part of its
          distribution, then any Derivative Works that You distribute must
          include a readable copy of the attribution notices contained
          within such NOTICE file, excluding those notices that do not
          pertain to any part of the Derivative Works, in at least one
          of the following places: within a NOTICE text file distributed
          as part of the Derivative Works; within the Source form or
          documentation, if provided along with the Derivative Works; or,
          within a display generated by the Derivative Works, if and
          wherever such third-party notices normally appear. The contents
          of the NOTICE file are for informational purposes only and
          do not modify the License. You may add Your own attribution
          notices within Derivative Works that You distribute, alongside
          or as an addendum to the NOTICE text from the Work, provided
          that such additional attribution notices cannot be construed
          as modifying the License.

      You may add Your own copyright statement to Your modifications and
      may provide additional or different license terms and conditions
      for use, reproduction, or distribution of Your modifications, or
      for any such Derivative Works as a whole, provided Your use,
      reproduction, and distribution of the Work otherwise complies with
      the conditions stated in this License.

   5. Submission of Contributions. Unless You explicitly state otherwise,
      any Contribution intentionally submitted for inclusion in the Work
      by You to the Licensor shall be under the terms and conditions of
      this License, without any additional terms or conditions.
      Notwithstanding the above, nothing herein shall supersede or modify
      the terms of any separate license agreement you may have executed
      with Licensor regarding such Contributions.

   6. Trademarks. This License does not grant permission to use the trade
      names, trademarks, service marks, or product names of the Licensor,
      except as required for reasonable and customary use in describing the
      origin of the Work and reproducing the content of the NOTICE file.

   7. Disclaimer of Warranty. Unless required by applicable law or
      agreed to in writing, Licensor provides the Work (and each
      Contributor provides its Contributions) on an "AS IS" BASIS,
      WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
      implied, including, without limitation, any warranties or conditions
      of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or FITNESS FOR A
      PARTICULAR PURPOSE. You are solely responsible for determining the
      appropriateness of using or redistributing the Work and assume any
      risks associated with Your exercise of permissions under this License.

   8. Limitation of Liability. In no event and under no legal theory,
      whether in tort (including negligence), contract, or otherwise,
      unless required by applicable law (such as deliberate and grossly
      negligent acts) or agreed to in writing, shall any Contributor be
      liable to You for damages, including any direct, indirect, special,
      incidental, or consequential damages of any character arising as a
      result of this License or out of the use or inability to use the
      Work (including but not limited to damages for loss of goodwill,
      work stoppage, computer failure or malfunction, or any and all
      other commercial damages or losses), even if such Contributor
      has been advised of the possibility of such damages.

   9. Accepting Warranty or Additional Liability. While redistributing
      the Work or Derivative Works thereof, You may choose to offer,
      and charge a fee for, acceptance of support, warranty, indemnity,
      or other liability obligations and/or rights consistent with this
      License. However, in accepting such obligations, You may act only
      on Your own behalf and on Your sole responsibility, not on behalf
      of any other Contributor, and only if You agree to indemnify,
      defend, and hold each Contributor harmless for any liability
      incurred by, or claims asserted against, such Contributor by reason
      of your accepting any such warranty or additional liability.

   END OF TERMS AND CONDITIONS

   APPENDIX: How to apply the Apache License to your work.

      To apply the Apache License to your work, attach the following
      boilerplate notice, with the fields enclosed by brackets "{}"
      replaced with your own identifying information. (Don't include
      the brackets!)  The text should be enclosed in the appropriate
      comment syntax for the file format. We also recommend that a
      file or class name and description of purpose be included on the
      same "printed page" as the copyright notice for easier
      identification within third-party archives.

   Copyright Individual contributors

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
```

### ISC

```text
Permission to use, copy, modify, and/or distribute this software for any purpose with or without
fee is hereby granted, provided that the above copyright notice and this permission notice appear
in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS
SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE
AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT,
NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE.
```

### Unicode License V3

```text
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 1991-2023 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a
copy of data files and any associated documentation (the "Data Files") or
software and any associated documentation (the "Software") to deal in the
Data Files or Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, and/or sell
copies of the Data Files or Software, and to permit persons to whom the
Data Files or Software are furnished to do so, provided that either (a)
this copyright and permission notice appear with all copies of the Data
Files or Software, or (b) this copyright and permission notice appear in
associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
THIRD PARTY RIGHTS.

IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall
not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written
authorization of the copyright holder.
```

### The Unlicense

```text
This is free and unencumbered software released into the public domain.

Anyone is free to copy, modify, publish, use, compile, sell, or
distribute this software, either in source code form or as a compiled
binary, for any purpose, commercial or non-commercial, and by any
means.

In jurisdictions that recognize copyright laws, the author or authors
of this software dedicate any and all copyright interest in the
software to the public domain. We make this dedication for the benefit
of the public at large and to the detriment of our heirs and
successors. We intend this dedication to be an overt act of
relinquishment in perpetuity of all present and future rights to this
software under copyright law.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
OTHER DEALINGS IN THE SOFTWARE.

For more information, please refer to <http://unlicense.org/>
```

## Keeping this file current

The table comes from `cargo metadata` plus the `LICENSE` file each crate ships in the cargo
registry, so it reflects `Cargo.lock` rather than anyone's memory. Adding or updating a
dependency means regenerating it:

```sh
pwsh -File scripts/third-party-notices.ps1
```

The five License texts are read from the crates that use them, so they are verbatim rather
than retyped. If one of those crates is ever dropped, point the generator at another crate
under the same License.

