# Mars upstream notices

These notices accompany Mars and its independently reusable Eye, V9 and Relay libraries.
Redistributing an individual library must retain the notices applicable to its source
and dependencies. Upstream Blink and V8 names below identify provenance, not runtime engines.

## Chrome DevTools Protocol definitions

`crates/mdp/protocol/` vendors the protocol definition files (`browser_protocol.pdl`,
`domains/*.pdl`) from Chromium `2c592105bbcd9490a9894df48d0fe59b2c512651`
(`third_party/blink/public/devtools_protocol/`) and `js_protocol.pdl` from V8
`fedd021b538607860f9c56e5c193ad8dcd4f2bb1` (`include/`), unmodified. They are data the
front door's catalog reads; no protocol code is generated from them. Copyright 2017 The Chromium
Authors; Copyright 2017 The Chromium Authors. All rights reserved. They are used under the
BSD-style terms below.

## V8 reference algorithms and adapted test cases

The V9 scanner and arithmetic-expression work studies V8 revision
`fedd021b538607860f9c56e5c193ad8dcd4f2bb1`. The source map in
[UPSTREAM.md](UPSTREAM.md) distinguishes reviewed reference code from implemented
behavior. Retain this notice with source and binary distributions containing
adaptations.

Source file notices: Copyright 2011 the V8 project authors. All rights reserved.
The reviewed parser base carries Copyright 2012 the V8 project authors.
All rights reserved.
The additionally inspected semicolon/trivia routines use `src/parsing/parser-base.h`
and `src/parsing/token.h` (Copyright 2012 the V8 project authors),
`src/parsing/scanner.cc`, `src/parsing/scanner.h` and `src/strings/unicode.h`
(Copyright 2011 the V8 project authors), and `src/parsing/scanner-inl.h`
(Copyright 2018 the V8 project authors). All rights reserved. They use the same
BSD-style terms below. UPSTREAM.md distinguishes complete named routines and the
complete unicode source from partial scanner/token file views. The bounded Rust
adaptation owns its scanner metadata and statement-tail policy; it does not port
V8's complete scanner, diagnostic normalization or grammar, or import an ASI
conformance suite.
The additionally reviewed `src/objects/contexts.h` carries Copyright 2012 the V8
project authors; `src/objects/js-function.tq` carries Copyright 2020 the V8 project
authors. All rights reserved. The closure/context reference adaptations and owned
Rust graph collector are distinguished in UPSTREAM.md.
The additionally reviewed interpreter bytecode generator carries Copyright 2015
the V8 project authors. All rights reserved. The parser and interpreter routines
are semantically adapted to bounded Rust AST evaluation, as recorded in UPSTREAM.md.
The additionally reviewed `src/builtins/arm64/builtins-arm64.cc` carries Copyright
2013 the V8 project authors. All rights reserved. Receiver conversion is semantically
adapted to bounded Rust evaluation; its machine-code generator is not ported.
The additionally reviewed `src/builtins/function.tq`, `src/objects/js-function.tq`
and `src/builtins/conversion.tq` carry Copyright 2020 the V8 project authors;
`src/builtins/builtins-function.cc` carries Copyright 2016 the V8 project authors.
All rights reserved. The callable-object layer semantically adapts reviewed
algorithms rather than their Torque, native accessor or machine-code machinery.
The additionally reviewed String and Array routines carry Copyright 2019 the V8
project authors (`string-trim.tq`), Copyright 2017 (`builtins-string-gen.cc`),
Copyright 2014 (`runtime-regexp.cc`), and Copyright 2018 (`array-map.tq`,
`array-filter.tq`, `array-join.tq`). All rights reserved. Their observable conversion,
iteration and string behavior guides the bounded Rust methods; V8 allocation,
optimization, Torque and generated-code machinery is not ported.
The additionally reviewed `src/runtime/runtime-scopes.cc` carries Copyright 2014
the V8 project authors. All rights reserved. Global declaration instantiation is
semantically adapted to the owned bounded Rust binding graph, as recorded in UPSTREAM.md.
The additionally reviewed `src/runtime/runtime-operators.cc` carries Copyright 2014
the V8 project authors; `src/builtins/array-some.tq` and `array-find.tq` carry Copyright
2018 the V8 project authors; `src/builtins/conversion.tq` carries Copyright 2020 the
V8 project authors. All rights reserved. Their inspected dispatch, conversion protocol
and selection loops guide bounded owned Rust behavior under the same BSD-style terms
below. The complete pinned Object::Equals body and current normative equality algorithm
were not read; UPSTREAM.md records the inspected proposal branches and inference rather
than claiming a complete core port. No optimized array/deoptimization implementation
or upstream some/find test suite is copied into Rust.
The additionally inspected array-literal and string-iteration routines carry Copyright
2015 the V8 project authors (`src/interpreter/bytecode-generator.cc`), Copyright 2017
(`src/builtins/builtins-iterator-gen.cc`, `builtins-string-gen.cc`), and Copyright 2019
(`src/builtins/string-iterator.tq`, `array-from.tq`). All rights reserved. They use the
same BSD-style terms below. UPSTREAM.md identifies full versus prefix source reads,
the uninspected ArrayIteratorPrototypeNext leaf and the owned-copy equivalence inference.
Rust semantically adapts bounded literal order, sparse materialization and UTF-16
code-point boundaries; it does not port iterator objects, generated code, optimization
or cleanup machinery, or import their conformance suites.
Adapted `test/mjsunit/number-tostring.js` cases: Copyright 2008 the V8 project
authors. All rights reserved.
The radix-formatting helper in `crates/v9/src/v9/value/number/radix.rs` adapts
`DoubleToRadixStringView` from `src/numbers/conversions.cc` at the same pinned
revision: Copyright 2011 the V8 project authors. All rights reserved. The
BSD-style redistribution terms below apply to this adaptation as well.
The additionally reviewed string protocol routines are
`src/builtins/string-match-search.tq` (Copyright 2021),
`src/builtins/string-startswith.tq` (Copyright 2018), and
`src/builtins/string-endswith.tq` (Copyright 2019), the V8 project authors.
All rights reserved. Their conversion and protocol order guides V9's owned
string helpers under the same BSD-style terms.

The applicable redistribution terms from that revision's `LICENSE` follow:

Copyright 2014, the V8 project authors. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

* Redistributions of source code must retain the above copyright
  notice, this list of conditions and the following disclaimer.
* Redistributions in binary form must reproduce the above
  copyright notice, this list of conditions and the following
  disclaimer in the documentation and/or other materials provided
  with the distribution.
* Neither the name of Google Inc. nor the names of its
  contributors may be used to endorse or promote products derived
  from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## ryu-js

`ryu-js` 1.0.3 is a Rust numeric-formatting primitive, licensed under
Apache-2.0 OR BSL-1.0. It supplies ECMAScript number-to-string formatting,
not a parser, virtual machine or browser engine. Its license files ship with
the Cargo dependency. This package retains the selected Boost Software License
at [licenses/ryu-js/LICENSE-BOOST](licenses/ryu-js/LICENSE-BOOST).

## rustls 0.23.45 (vendored)

`vendor/rustls` is the registry crate rustls 0.23.45, Apache-2.0 OR ISC OR MIT (its licence
files are alongside), with one additive change for Chrome-shaped ClientHellos. `vendor/rustls/MARS.md`
lists the changed files and how to diff against upstream.

## Image decoders

`png` 0.17.16 and `jpeg-decoder` 0.3.2 are licensed under MIT OR Apache-2.0.
Their license texts ship with the Cargo dependencies; the package uses their
bounded PNG and JPEG decoding APIs for static page images.

## Font and content decoders

`woff2-patched` 0.4.0 (Apache-2.0; it pulls in `brotli`, `bitvec`, `four-cc`,
`safer-bytes` and `thiserror`) unpacks WOFF2 web fonts; WOFF 1.0 tables are
inflated with `flate2` 1.1.10 (MIT OR Apache-2.0, Rust backend). The unpacked
sfnt is handed to CoreText per page and never registered system-wide.
`flate2`, `brotli-decompressor` 6.0.1 and `ruzstd` 0.9.1 (MIT OR Apache-2.0 /
BSD-3-Clause / MIT) decode gzip, deflate, brotli and zstd response bodies
under Relay's body bounds. `psl` 2.1.245 (MIT OR Apache-2.0) supplies the
public suffix list that cookie domain rules consult. Their license texts ship
with the Cargo dependencies.

## Blink BSD event-listener reference algorithms

The document bridge semantically adapts listener snapshot/removed-record and
callback identity/order behavior from Chromium revision
`2c592105bbcd9490a9894df48d0fe59b2c512651`, files
`third_party/blink/renderer/core/dom/events/event_target.cc` and
`event_listener_map.cc`. The applicable source notices follow. LGPL-covered
Event, dispatcher, IDL and button files were behavior inspection only; their
implementation is not copied or translated into this Rust bridge.

Copyright (C) 1999 Lars Knoll (knoll@kde.org)
Copyright (C) 1999 Antti Koivisto (koivisto@kde.org)
Copyright (C) 2001 Dirk Mueller (mueller@kde.org)
Copyright (C) 2004, 2005, 2006, 2007 Apple Inc. All rights reserved.
Copyright (C) 2006 Alexey Proskuryakov (ap@webkit.org)
Copyright (C) 2007, 2008 Nikolas Zimmermann <zimmermann@kde.org>
Copyright (C) 2011 Andreas Kling (kling@webkit.org)

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:
1. Redistributions of source code must retain the above copyright
   notice, this list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright
   notice, this list of conditions and the following disclaimer in the
   documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY APPLE COMPUTER, INC. ``AS IS'' AND ANY
EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL APPLE COMPUTER, INC. OR
CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY
OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## Blink BSD flex and sizing reference algorithms

The flex allocator, line breaking, box sizing and shorthand behavior semantically
adapt Chromium revision `2c592105bbcd9490a9894df48d0fe59b2c512651`:
`flex_layout_algorithm.cc` (Copyright 2018 The Chromium Authors),
`flex_item.h`, `line_flexer.cc`, `flex_line_breaker.cc` (Copyright 2024 The Chromium
Authors), `shorthands_custom.cc` (Copyright 2019 The Chromium Authors), and
`length_utils.cc` (Copyright 2016 The Chromium Authors). The LGPL-covered
`layout_block.cc` anonymous-block behavior was inspected only; its implementation
is not copied or translated. These modern reference files use Chromium's BSD terms:

Copyright 2015 The Chromium Authors

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

* Redistributions of source code must retain the above copyright
  notice, this list of conditions and the following disclaimer.
* Redistributions in binary form must reproduce the above
  copyright notice, this list of conditions and the following disclaimer
  in the documentation and/or other materials provided with the
  distribution.
* Neither the name of Google LLC nor the names of its
  contributors may be used to endorse or promote products derived from
  this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## Blink static-reading CSS reference algorithms

The bounded font, relative-length, normal-block margin and canvas implementation
semantically adapts inspected behavior at Chromium revision
`2c592105bbcd9490a9894df48d0fe59b2c512651`. No Blink engine, C++ object model,
resolver machinery or paint pipeline is embedded. The exact routine/access scope
and specification-based decisions are recorded in `UPSTREAM.md`.

The previously listed Chromium BSD notice also applies to these reference headers:

- `shorthands_custom.cc`: Copyright 2019 The Chromium Authors.
- `length_utils.cc`: Copyright 2016 The Chromium Authors.
- `css_length_resolver.cc`: Copyright 2022 The Chromium Authors.
- `box_background_paint_context.cc` and `font_size_style.h`: Copyright 2023 The Chromium Authors.
- `view_painter.cc`: Copyright 2014 The Chromium Authors; partial behavior inspection only.

`style_builder_converter.cc` and `css_to_length_conversion_data.h` retain the
following Google source notice. The full converter was acquired after its Oya
view truncated; the provenance record does not claim an Oya complete-file read.
The LGPL-covered `style_resolver_state.cc` (Lars Knoll 1999; Apple 2003–2011) was
inspected only; its implementation is not copied or translated into this slice.

Copyright (C) 2013 Google Inc. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

* Redistributions of source code must retain the above copyright
  notice, this list of conditions and the following disclaimer.
* Redistributions in binary form must reproduce the above
  copyright notice, this list of conditions and the following disclaimer
  in the documentation and/or other materials provided with the
  distribution.
* Neither the name of Google Inc. nor the names of its
  contributors may be used to endorse or promote products derived from
  this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## MCP protocol references

The Rust MCP adapter independently implements the versioned 2025-11-25 protocol
contracts documented in `UPSTREAM.md`; it does not copy protocol schemas or SDK
implementation code. The protocol repository distinguishes Apache-2.0 specification
materials, historical MIT materials and CC-BY-4.0 non-specification documentation.
The optional interoperability check uses the repository's existing official Node
SDK dependency; its dependency notices remain with that installed package.

## macOS Keychain bindings

`security-framework` 3.7.0 and `security-framework-sys` 2.17.0 provide Rust
bindings to macOS Security.framework for the default desktop cookie profile.
They are licensed MIT OR Apache-2.0; their license texts ship with Cargo
dependencies.

## Unicode 17 emoji data extracted from Test262

The generated `crates/v9/src/v9/regexp/emoji17.txt` derives from Test262's Unicode 17
generated regular-expression property lists. Those files carry Copyright 2025 Mathias
Bynens and the Test262 BSD license below. The underlying Unicode data is Copyright
© 2025 Unicode, Inc. and covered by the Unicode License V3 reproduced below.

Copyright (C) 2012 Ecma International
Copyright (C) 2025 Mathias Bynens
All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are
permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of
   conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice, this list
   of conditions and the following disclaimer in the documentation and/or other materials
   provided with the distribution.
3. Neither the name of the authors nor Ecma International may be used to endorse or
   promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE ECMA INTERNATIONAL "AS IS" AND ANY EXPRESS OR IMPLIED
WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL ECMA INTERNATIONAL BE
LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## ICU4X Unicode data

V9 uses pinned ICU4X `icu_normalizer` and `icu_properties` 2.3.0 for
Unicode normalization and exact regular-expression property sets. The crates
were already workspace dependencies through IDNA. Their Unicode-3.0 notice
also applies to the linked compiled Unicode data:

UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 2020-2024 Unicode, Inc.

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

SPDX-License-Identifier: Unicode-3.0

—

Portions of ICU4X may have been adapted from ICU4C and/or ICU4J.
ICU 1.8.1 to ICU 57.1 © 1995-2016 International Business Machines Corporation and others.

## Unicode simple case-folding data (regex-syntax 0.8.11)

`crates/v9/src/v9/regexp/case_data.rs` adapts the Unicode 16.0 generated simple case-folding table from regex-syntax 0.8.11. The Unicode 17.0 supplement consists of mutually inverse single-scalar lowercase/uppercase pairs from Rust 1.94.0 (whose `char::UNICODE_VERSION` is 17.0.0). Special folds retain the complete generated table. The table is used as data by V9's own matcher; no foreign regular-expression engine is used.

```text
Copyright (c) 2014 The Rust Project Developers

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```
