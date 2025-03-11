FROM ghcr.io/gleam-lang/gleam:v1.8.1-erlang-alpine

COPY . /build/

RUN apk add gcc build-base \
  && cd /build \
  && gleam export erlang-shipment \
  && mv build/erlang-shipment /app \
  && rm -r /build \
  && apk del gcc build-base \
  && addgroup -S flyradar \
  && adduser -S flyradar -G flyradar \
  && chown -R flyradar /app

USER flyradar 

ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["run"]
