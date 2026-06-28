from fastapi import FastAPI

app = FastAPI()


@app.get("/products")
def list_products():
    return []


@app.post("/products")
def create_product():
    return {}


@app.get("/products/{product_id}")
def get_product(product_id: int):
    return {}
