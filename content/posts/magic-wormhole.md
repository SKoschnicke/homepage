+++
title = "Magic Wormhole"
tags = ["software"]
draft = false
+++

In a world where software gets more and more complex, and because of that also bloated, there are sometimes little gems which make your day a little bit better. They adhere to the initial goal of software and computers, helping people to get their tasks done. And when they just do their job and then get out of the way, it sometimes makes me smile.

One if these programs I just had the pleasure to use again is Magic Wormhole. A small command line utility which lets you transfer files between two computers.

![](/ox-hugo/2025-05-07_16-41-33_screenshot.png)
[XKCD "File transfer"](https://xkcd.com/949/)

So I wanted to give my laptop SSH access to a server I already had access to from my desktop computer. Of course, login through password is deactivated on the server. So I had to add the public SSH key of my laptop to the `authorized_keys` file on the server.

I could have sent my public key to myself by email or even put it on an USB stick and get it onto the desktop computer like this and then copy it to the server (using the very handy `ssh-copy-id` program), but  I remembered Magic Wormhole, which would let me transfer the key more easily to my desktop computer.

I had it already installed, but it should be easy to install on almost any system (don't ask me about Windows, though). Trying to remember how to use it, I entered `wormhole --help` (actually I first tried `wormhole -h`, but that doesn't work):

```text
$ wormhole --help
Usage: wormhole [OPTIONS] COMMAND [ARGS]...

  Create a Magic Wormhole and communicate through it.

  Wormholes are created by speaking the same magic CODE in two different
  places at the same time.  Wormholes are secure against anyone who doesn't
  use the same code.

Options:
  --appid APPID                   appid to use
  --relay-url URL                 rendezvous relay to use
  --transit-helper tcp:HOST:PORT  transit relay to use
  --dump-timing FILE.json         (debug) write timing data to file
  --version                       Show the version and exit.
  --help                          Show this message and exit.

Commands:
  help
  receive  Receive a text message, file, or directory (from 'wormhole send')
  send     Send a text message, file, or directory
  ssh      Facilitate sending/receiving SSH public keys
```

Okay, so executing `wormhole send .ssh/id_ed25519.pub` should give me a code-word which I can use on the receiving machine by executing `wormhole receive` and get the file securely transferred.

But then the `ssh` command caught my attention, as it seemed to be exactly my use-case:

```text
$ wormhole ssh --help
Usage: wormhole ssh [OPTIONS] COMMAND [ARGS]...

  Facilitate sending/receiving SSH public keys

Options:
  --help  Show this message and exit.

Commands:
  accept  Send your SSH public-key
  invite  Add a public-key to a ~/.ssh/authorized_keys file
```

That was perfect. Just executing `wormhole ssh invite` on the server:

```text
$ wormhole ssh invite
Now tell the other user to run:

wormhole ssh accept 1-embezzle-printer
```

And then running `wormhole ssh accept 1-embezzle-printer` on the laptop:

```text
$ wormhole ssh accept 1-embezzle-printer
Sending public key type='ssh-ed25519' keyid='wayreth ssh key'
Really send public key 'wayreth ssh key' ? [y/N]: y
Key sent.
```

And on the server the program exits with:

```text
Appended key type='ssh-ed25519' id='wayreth' to '/home/sven/.ssh/authorized_keys'
```

That's it. Now I could access the server from my laptop. Easy, secure, straight forward.
