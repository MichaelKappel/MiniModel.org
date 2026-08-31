# SLM2 Composition Identity

One TinyRustLM model installation contains exactly:

- `model.slm2`
- `tokenizer.tokenizer2`
- `template.template2`
- `sampling.sampling2`
- `prompt.prompt2`
- `composition.acg2`

A MiniModel catalog entry binds every member's logical name, byte length,
SHA-256 digest, schema identity, semantic role, public source revision and
source lock, conversion and quantization profiles, runtime profile, and
qualification evidence. Paths and display aliases are not identity.

The six-member set remains closed. Separately, each qualified catalog entry
must bind one canonical `tinyrustlm.browser-composition-admission/1` metadata
document through an exact same-origin route, byte count no greater than 64 KiB,
and prefixed SHA-256 digest. This admission carries browser runtime and
activation policy; it is not a composition member and MiniModel.org serving the
metadata does not authorize model-byte hosting or proxying.

Missing, duplicate, extra, overlapping, substituted, or cross-composition
members fail closed. An exact piece match proves only those transferred bytes;
it does not substitute for final whole-member hashing, composition closure,
numerical parity, browser execution, or behavioral qualification.
