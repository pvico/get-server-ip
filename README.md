# get-server-ip

A utility to find the (dynamic) IP of a server.

It will

- check the Github gist file containing the IP
- check all the dynamic DNS services addresses pointing to this IP
- do that concurrently and with a time bound duration
- return the IP if at least the Github gist file IP and one DDNS service IP are the same
- return the IP matching at least two of the DDNS services if there is no match between the Github gist file IP and one dns service IP (e.g. if the Github gist file IP is not available)

These IP sources are updated by some external automation (e.g. n8n workflow, node-red, etc.).

Zig version 0.15.1

## Configuration

Configure the gist file URI and the DDNS service domains URI's in the `config.json` file. 
This file *must* be located in a subdirectory `.get-server-ip` of the cli executable location.