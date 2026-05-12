"""FastAPI application demonstrating endpoint extraction."""
from fastapi import FastAPI, APIRouter
from pydantic import BaseModel

app = FastAPI()
router = APIRouter(prefix="/api/v1")


class UserCreate(BaseModel):
    email: str
    name: str


@app.get("/health")
def health_check():
    return {"status": "ok"}


@app.post("/users")
def create_user(payload: UserCreate):
    return {"id": 1, "email": payload.email}


@router.get("/items/{item_id}")
async def get_item(item_id: int):
    return {"id": item_id}


@router.delete("/items/{item_id}")
async def delete_item(item_id: int):
    return None


app.include_router(router)
