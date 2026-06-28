FROM python:3.12-slim
WORKDIR /code
COPY . .
RUN pip install -e .
EXPOSE 8000
CMD ["uvicorn", "main:app", "--host", "0.0.0.0"]
