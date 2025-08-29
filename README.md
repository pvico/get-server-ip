# getip

A utility to find the (dynamic) IP of the Raspberry Pi 5 server as updated by the n8n workflow

It will check

- the Github gist file containing the IP
- all the dynamic DNS services that are updated with this IP
- do that concurrently and with a time bound duration
- return the IP if at least the Github gist file IP and one DDNS service IP are the same
- If there is no match between the Github gist file IP and one dns service IP (e.g. the Github gist file IP is not available), return the IP matching at least twao of the DDNS services

Configure the gist file URI and the DDNS services URI's in a `config.json` file located in a subdirectory `.get-server-ip` in the same path as the cli executable