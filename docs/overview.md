# Unpack

Unpack is a Task tracker for me. I usually get a mountain of new tasks everyday by my manager. and i usually work on as many as i can but these are big tasks and sometimes i have to prioritize them. this sometimes causes me to forget those tasks and i miss out on them. I need some way to track them. So basically i need a smart to do list that can act as my assistant to help me get a good overview of my tasks, workload and what should i work on next.

## Core Idea

This Task Tracker should help me just throw in context and it should create those tasks based on the context. it should automatically type create those tasks. Based on the quality of the context it should be able to set priorities, catagorize them, create sub-tasks, organize them, if they are related to some other larger prexisting tasks. also they should be able to modify old tasks if they are mentioend in the context.

## Context

I usually get these tasks in meetings, teams message, email or sometimes it's not tracked and i just know them that i need to do them. So context can be any of that. I should be able to drop files or type in new task (maybe it's just a description of it). At a time I should be able to add multiple context files and also a context file might contain different tasks as well.

## Model

This should be simple enough, so I think we can run a Haiku Model in the backend to go through those context files and maintain tasks.

## Tech

Ideally I would like a native app built with Tauri (Rust + React). It should look very clean, modern and professional. Make it look stylish but intuitive. The app should have a simple look and things should be easy to find and look at. Too much information can overwhelm a user so follow the best ui practices while building this.
