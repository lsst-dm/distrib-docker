FROM golang:latest as installer
RUN git clone https://github.com/GoogleCloudPlatform/gcsfuse.git
WORKDIR /go/gcsfuse
RUN go build -o gcsfuse
#FROM nginx
FROM nginxinc/nginx-unprivileged
USER root
RUN apt-get update && apt-get install fuse -y
COPY --from=installer /go/gcsfuse/gcsfuse /bin/
#COPY ./nginx.conf /etc/nginx/sites-enabled/default
#COPY ./nginx.conf /etc/nginx/nginx.conf
COPY ./nginx.conf /etc/nginx/conf.d/default.conf
USER nginx
#RUN mkdir /eups
COPY script.sh .
EXPOSE 8080
CMD ["sh", "script.sh"]
