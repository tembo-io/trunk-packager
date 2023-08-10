# shared-lib-finder

Download all extension archives from Trunk and then check what shared libraries they link to.

## Example

```
- Libraries for pgmq
        * - libgcc_s.so.1
        * - libc.so.6
        * - ld-linux-x86-64.so.2
- Libraries for postgis_sfcgal
        * - libgeos_c.so.1
        * - libproj.so.22
        * - libSFCGAL.so.1
        * - libm.so.6
        * - libc.so.6
- Libraries for pgroonga
        * - libgroonga.so.0
        * - libc.so.6
```