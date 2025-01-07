# kv-downloader

[![CI](https://github.com/subdigital/kv-downloader/actions/workflows/ci.yml/badge.svg)](https://github.com/subdigital/kv-downloader/actions/workflows/ci.yml)

This is a small utility that automates a workflow for downloading individual tracks from Karaoke Version.

This workflow is specific to how I use this service. If you want to change it you're welcome to fork this
repo and make your own changes. I _may_ accept pull requests if the changes are useful and general enough.

## What it does

This app will drive a headless (or visible) Chromium browser that will log into your account, navigate to
a song page. It will solo & download each individual track separately.

The browser portion of this app will auto-download upon first use.

## Why?

I like to set up my own mix for backing tracks using Logic. For maximum flexibility I want each track downloaded separately.

## Requirements

- macOS, Linux, Windows
- Karaoke Version account with purchased songs
- Chromium (will be downloaded automatically)

## Installation & Set Up

Look at the Releases page’s assets for the latest version of the app. Download the appropriate version for your platform,
uncompress it, and put it somewhere in your PATH.

> [!IMPORTANT]
> Note for macOS: Gatekeeper may block the app from running. In order to run this, navigate to Privacy & Security in System Preferences, and click the "Open Anyway" button.

Run `kv_downloader auth` to provide your credentials. You only need to do this once.

> [!NOTE]
> Your credentials are stored securely by your operating system's keychain. They are not sent anywhere _except_ to the Karaoke Version website.

## Usage

First, you have to purchase the track in your Karaoke Version account. Copy the URL of the song you want.

Then run `kv_downloader <song url>`. You can also pass options to customize the behavior:

## Options

- `-d <path>` - Change the download location
-  `-h` or `--headless` - Use headless mode, which hides the UI.
-  `-t <transpose offset>` - Change the pitch of the downloaded tracks (-1 to go down half step, 1 to go up half step, etc)
- `--count-in` - Include the intro precount on all tracks
- `--debug` - Enable debug logging (in case something goes wrong this helps give more detail)

Using headless mode may make it less clear what is going on behind the scenes, so I suggest testing it out
in the regular mode first.


## Build and Run from Source

- Ensure you have a [working rust development setup](https://www.rust-lang.org/learn/get-started)
- Clone or download the zip file of the source code
- cd into the project and type `cargo build`
- Add a `.env` file in the root of the project and include:

```
KV_USERNAME=<yourusername>
KV_PASSWORD=<yourpassword>
```

- type `cargo run -- --help` (note the double dashes to separate the args from the cargo command)

Usage is the same as above, except you'll be typing `cargo run --` instead of `kv_downloader`.

For example:

```
cargo run -- <song url> -d my_song_dir --count-in
```

## Use at your own risk

This tool is simply to automate a process that you would do normally. I do not recommend using this
in huge batches or concurrently, as this may well be against their terms, so use at your own risk.

I would hate for anyone's account to get banned for abusing automation like this.

And Karaoke Version, if you're listening: We'd love if this was fully supported in the UI!

## License

This source code is released under the MIT license.
