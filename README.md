# getip

A utility to find the (dynamic) IP of the Raspberry Pi 5 server as updated by the n8n workflow

It will check

- the Github gist wanip.txt
- all the dynamic DNS services that are updated with this IP
- do that concurrently and with a time bound duration
- return the IP if at least the wanip.txt and one dns service are the same