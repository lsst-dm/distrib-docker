FROM golang:latest as installer
RUN git clone https://github.com/GoogleCloudPlatform/gcsfuse.git
WORKDIR /go/gcsfuse
RUN go build -o gcsfuse
FROM nginx:latest
ENV MNT_DIR="/eups"
RUN apt-get update && apt-get install fuse -y
COPY --from=installer /go/gcsfuse/gcsfuse /bin/
COPY ./nginx.conf /etc/nginx/sites-enabled/default
RUN mkdir "$MNT_DIR"
COPY script.sh .
CMD ["sh", "script.sh"]
