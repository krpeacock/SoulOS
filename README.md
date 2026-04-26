# Soul OS

Soul OS is a minimalistic operating system that is inteded to run on old devices, in particular to be friendly to e-ink readers. Therefore it is an intentionally restrictive UI scheme, with a design philosophy inspired by Palm OS.

## Running locally

If you are on MacOS, you will need to install SDL2. I recommend using Homebrew.

`brew install sdl2`

You also will need cargo. 

`brew install rustup`

`rustup install cargo`

Then to run the application, run 

`cargo run`

## Running on Android

There's a `soul-runner-android` cdylib + `cargo-apk` setup for sideloading
a debug APK onto a real device. This is for development only — no
production signing, no Play Store packaging.

Install once:

```
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo install cargo-apk --locked
# Point the env at your Android NDK:
export ANDROID_HOME=/path/to/android-sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>
```

Build + install:

```
cargo apk run -p soul-runner-android --release
```

CI builds an APK on every push to `main` (and on tag pushes) via
`.github/workflows/android.yml`; grab it from the workflow artifacts.

## Aspirations

Eventually, the goal is to have this be software that can be run on bare metal. I would like to try to run it on an old Galaxy S4, or a deprecated Kindle.

In the shorter term, I'd like to ship it as an app people can play with on mobile devices, and potentially as an Android launcher, that can be used to launch other installed applications. But first it can be a playground in its own right.
