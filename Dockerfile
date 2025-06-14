FROM httpd:2.4

RUN apt update \
    && apt upgrade -y \
    && apt install -y libcap2-bin \
    && setcap 'cap_net_bind_service=+ep' /usr/local/apache2/bin/httpd \
    && chown www-data:www-data /usr/local/apache2

RUN setcap 'cap_net_bind_service=+ep' /usr/local/apache2/bin/httpd
RUN getcap /usr/local/apache2/bin/httpd

COPY httpd.conf /usr/local/apache2/conf/httpd.conf
USER www-data

