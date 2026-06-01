+++
title = "Overengineering a static website"
author = ["Sven Koschnicke"]
description = "A cautionary tale about building a Rube Goldberg machine when a hammer would do."
date = 2026-02-23
tags = ["web", "rust", "cloud", "unikernel"]
draft = false
+++

## The Boring Way {#the-boring-way}

In the beginning, there was static HTML served by Apache. Then everyone wanted dynamic content, so we got CGI, then PHP, and suddenly every website was dynamic — even the ones that didn't need to be. Today, static site generators have brought us full circle. Write some Markdown (or org-mode, because it's better), let Hugo convert it to HTML, push to GitHub, done. I did exactly that with the first versions of this homepage. It works well. The Hugo conversion can even run in a GitHub workflow.


## The Custom Server {#the-custom-server}

But it's also a little bit boring. I thought about how a generic webserver like Nginx could never be optimized to such an extend for serving my static website because it needs to be usable for other use-cases, too. I wanted to try if it would be faster to keep all the pages in memory the way they need to be sent to the client. So I wrote a custom webserver in Rust which gets all the content compiled into the binary on build time. Then I did some benchmarks. The Rust server was over 16 times faster than a standard Nginx server.

**Test Environment:**

-   Hugo site: ~6.4MB, 89 files
-   Benchmark tool: wrk (4 threads, 100 connections, 30 seconds)
-   Rust server: Port 3000 (optimized release build)
-   nginx: Port 8080 (Alpine, podman container, default config)

**Results Summary**

| Test Case           | Rust Req/s | nginx Req/s | Speedup   | Rust Latency | nginx Latency |
|---------------------|------------|-------------|-----------|--------------|---------------|
| Homepage (HTML)     | 122,519    | 7,261       | **16.8x** | 754μs        | 16.95ms       |
| CSS (fingerprinted) | 129,547    | 8,791       | **14.7x** | 712μs        | 15.92ms       |
| PNG Image (large)   | 28,155     | 10,494      | **2.7x**  | 2.15ms       | 16.81ms       |

That was promising, so I added TLS support to the server. When comparing the deployed custom server with GitHub pages, Github pages still wins in overall response speed. That is because the most time of a web request is not spent waiting for the webserver but transferring the data from the server to the client, because the user is rarely sitting right next to the datacenter where the server is located.

To make the distance between server and client as short as possible, one uses a content delivery network (CDN). The static content is copied to multiple datacenters all over the world and served from the location nearest to the user. GitHub pages distributes the content through a CDN by default and that makes it faster than any single webserver in one location (except when the user happens to be near this single server).

But what a CDN can't do is serve dynamic content. So I added a dynamic component to my homepage in the form of live statistics about requests to the server. It is nothing really useful, but it is something a CDN cannot do. You can see the statistics in action below each page. Click on "Show more" to see some diagrams. I made sure to only use data that cannot identify or track you. For now, the number of concurrent page requests (in a one second window) and the response time (that is, how long the server needs to respond to the request, not how long it takes response to reach the client) are measured. No IPs or other information are stored.


## Going Full Unikernel {#going-full-unikernel}

I could have stopped here. Deploy the Rust server to a virtual private server and be happy. But I didn't want to stop. The server was already a single binary also containing the content it should serve. So why not skip the whole operating system and deploy the server as a unikernel directly to the hypervisor? The most difficult thing was actually finding a cloud provider that supported that and understanding the API for deployment. I chose Hetzner, because they are based in Europe, but it shows that unikernels are a not so often used technology.

Using the API successfully needed some experimentation, and the `ops` tool to build an unikernel image needs some polishing, but eventually it worked.

TLS certificates from Let's Encrypt couldn't be stored on disk anymore, so I had to use a storage bucket for that (Hetzner provides these with a S3 compatible API). Storing the certificates only in memory would have meant that every deploy needs a new certificate, and that way I might have run into rate limits from Let's Encrypt.

But now everything works. This website is served from a custom server running directly on the hypervisor. No system updates I have to worry about (the provider cares about the hypervisor), I only have to update the server itself. And it is fast. I also spent some time reducing the amount of data transferred by optimizing the binary files (images, PDFs). The server supports brotli compression (in addition to the standard gzip compression).

It's still slower than using a CDN if you are not near the datacenter, but everything is properly optimized now and it was a fun learning experience.


## Recap {#recap}

Is this kind of deployment actually better? Speed and reduced attack surface are the advantages. I can also add fun little gimmicks like the statistics, because I control the whole webserver, but there are also not so nice things:

-   deployment is slow. The binary needs to be built, then the image needs to be built from the binary, then the image needs to be started in the cloud. Takes about 5 to 10 minutes until the new version is online. That's a long time for small content changes.
-   no easy debugging in production. I can't SSH into the server or look at logs (there are no logs, currently). I'd need to create observability infrastructure like OpenTelemetry and some log consumer where the server could send data. Or I'd need to integrate that into the server, extending its attack surface again.
-   the allocated server resources are way too many for a simple static website. I'm paying about 3.50 euros for the server (2 vCPUs, 4 GB RAM) plus about 6 euros for object storage (that's basically Hetzner's base fee, I'm only using a few kilobytes for the certificates).
-   I can't share the server resources to run other services like a Mastodon server or a Matrix server

Should static sites be served like this? No, using something like GitHub pages is easier and faster. Maybe I will go from unikernel deployment back to a virtual private server where the binary runs and change the server so that it can dynamically update the in-memory content. That would make better use of the resources and reduce the time it takes to update the content drastically. But in the end, it's just some fun experiment.
