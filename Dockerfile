FROM nginxinc/nginx-unprivileged
USER root
RUN apt-get update && apt-get install fuse -y
# COPY ./nginx.conf /etc/nginx/conf.d/default.conf
COPY ./nginx.conf /etc/nginx/nginx.conf
USER nginx
COPY style.xsl .
COPY script.sh .
EXPOSE 8080
CMD ["sh", "script.sh"]
