# Interoperability and reverse engineering policy

This project is an independent, unofficial compatibility project. This policy
summarizes relevant EU rules for contributors; it is not legal advice, does not
determine whether a particular activity is lawful, and is not a guarantee of
legal clearance. The rules below have distinct scopes and may depend on national
law and the facts. Seek qualified legal advice where the legal basis or
distribution rights are uncertain.

## Compatibility work

Prefer independently authored implementations based on documented interfaces
or information obtained through lawful observation, study and testing of
software or devices the contributor is entitled to use. Do not copy protected
expression, vendor code, leaked material or proprietary SDK content.

EU law includes limited exceptions relevant to interoperability:

- **Software observation and testing:** Article 5(3) of
  [Directive 2009/24/EC](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32009L0024)
  permits a person entitled to use a copy of a program to observe, study or test
  its functioning to identify underlying ideas and principles while performing
  acts they are entitled to perform.
- **Decompilation for software interoperability:** Article 6 of that Directive
  allows reproduction or translation of code, without the rights holder's
  authorization, only where indispensable to obtain information needed for
  interoperability of an independently created program and subject to its
  conditions. These include entitlement to use the program, the information
  not already being readily available, limiting the acts to necessary parts,
  and restrictions on the purpose, use and disclosure of the information.
- **Contract terms:** Article 8(2) provides that contractual provisions
  contrary to Article 6 or the specified Article 5 exceptions are null and
  void. Do not assume that every restriction is invalid; identify the specific
  clause, activity and exception and request case-specific review if disputed.
- **Trade secrets and products:** Article 3(1)(b) of
  [Directive (EU) 2016/943](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32016L0943)
  addresses acquisition of trade secrets through observation, study,
  disassembly or testing of a product or object made available to the public or
  lawfully in the acquirer's possession. It is subject to the provision's
  conditions, including any legally valid duty to limit acquisition.

These are not a general right to reverse engineer any product, access any
system, or publish all resulting information. A software copyright exception
does not automatically resolve trade-secret, patent, contract, privacy,
security, or other legal issues. Open-source licensing or publication neither
creates nor expands an exception. Publish only original code and necessary
interoperability facts that have appropriate provenance, and do not disclose
restricted information or use it beyond what applicable law permits.

## Project-specific safety boundary

Regardless of whether an interoperability exception might apply, this project
does not authorize contributors to bypass authentication, pairing approvals,
encryption, access controls, DRM, firmware protections or vendor service
restrictions. Do not infer authorization from device ownership or compatibility
purpose. Stop and request maintainer review before work involving uncertain
authorization, disputed terms, restricted materials or a potentially
circumvented protection.

## Evidence and documentation

Document interoperability facts in original wording, with source links,
revisions and applicable model/firmware scope. Distinguish documented
information, authorized observations and unverified assumptions. Keep evidence
minimal and sanitized; do not publish raw captures, credentials, identifiers,
private observations or proprietary materials. Follow the repository-wide
provenance and sensitive-data requirements in [AGENTS.md](AGENTS.md).
