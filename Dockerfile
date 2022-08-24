FROM nginx:latest
COPY ./nginx/nginx.conf /etc/nginx/nginx.conf
COPY ./nginx/mime.conf /etc/nginx/conf.d/
COPY ./nginx/index.html /usr/share/nginx/html
COPY ./nginx/style.css /usr/share/nginx/html
COPY ./wasm /usr/share/nginx/html
EXPOSE 80

ENTRYPOINT [ "nginx", "-g", "daemon off;" ]