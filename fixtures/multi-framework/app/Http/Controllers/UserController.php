<?php

namespace App\Http\Controllers;

use App\Models\User;
use Symfony\Component\Routing\Annotation\Route;

class UserController extends Controller
{
    /** List every user. */
    public function index()
    {
        return User::all();
    }

    #[Route('/api/users/{id}', methods: ['GET'])]
    public function show(int $id)
    {
        return User::find($id);
    }
}
