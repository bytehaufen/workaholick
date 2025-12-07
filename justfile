doc:
  # cargo doc --open
  cargo doc --no-deps
  xdg-open http://0.0.0.0:8000/workaholick/index.html
  simple-http-server target/doc
